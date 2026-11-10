use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use paint_domain::ports::PersistenceError;
use paint_domain::{
    CanvasSize, Color, DEFAULT_HISTORY_LIMIT, Document, DomainError, DrawOp, EditCommand, History,
    LayerContent, LayerId, PixelRect, Point, StrokeWidth, Underlay,
};
use paint_engine::{LayerCache, RenderOptions, compose_visible};
use paint_tools::{ToolController, ToolEvent, ToolKind, ToolSettings};

use crate::Services;

/// Error reported to the user by the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppError {
    Domain(DomainError),
    Persistence(PersistenceError),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Domain(e) => e.fmt(f),
            Self::Persistence(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for AppError {}

impl From<DomainError> for AppError {
    fn from(e: DomainError) -> Self {
        Self::Domain(e)
    }
}

impl From<PersistenceError> for AppError {
    fn from(e: PersistenceError) -> Self {
        Self::Persistence(e)
    }
}

/// What a PNG export contains.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportMode {
    /// All visible layers composed (transparent where nothing is drawn).
    VisibleLayers,
    /// Drawing layers only, background transparent.
    DrawingOnly,
}

/// The ViewModel of the main window.
#[derive(Debug)]
pub struct PaintViewModel {
    services: Services,
    document: Document,
    history: History,
    tools: ToolController,
    render_options: RenderOptions,
    caches: BTreeMap<LayerId, LayerCache>,
    /// Incremented whenever the underlay image or canvas size changes.
    underlay_revision: u64,
    /// Ids of the states on the undo / redo stacks (for "modified" tracking).
    undo_states: Vec<u64>,
    redo_states: Vec<u64>,
    next_state: u64,
    saved_state: u64,
    project_path: Option<PathBuf>,
}

impl PaintViewModel {
    pub fn new(services: Services) -> Self {
        Self::with_document(services, Document::new(CanvasSize::DEFAULT))
    }

    pub fn with_document(services: Services, document: Document) -> Self {
        let mut vm = Self {
            services,
            document,
            history: History::new(DEFAULT_HISTORY_LIMIT),
            tools: ToolController::default(),
            render_options: RenderOptions::default(),
            caches: BTreeMap::new(),
            underlay_revision: 0,
            undo_states: Vec::new(),
            redo_states: Vec::new(),
            next_state: 1,
            saved_state: 0,
            project_path: None,
        };
        vm.rebuild_all_caches();
        vm
    }

    // ----- state for the view -----

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn canvas(&self) -> CanvasSize {
        self.document.canvas()
    }

    /// Display raster of a vector layer (committed content + preview).
    pub fn layer_cache(&self, id: LayerId) -> Option<&LayerCache> {
        self.caches.get(&id)
    }

    /// Area of a vector layer changed since the last call (for partial
    /// texture updates).
    pub fn take_layer_dirty(&mut self, id: LayerId) -> Option<PixelRect> {
        self.caches.get_mut(&id).and_then(LayerCache::take_dirty)
    }

    pub fn underlay_image(&self) -> Option<&Arc<paint_domain::RasterImage>> {
        self.document.underlay_layer().and_then(|l| l.image())
    }

    pub const fn underlay_revision(&self) -> u64 {
        self.underlay_revision
    }

    pub const fn tool(&self) -> ToolKind {
        self.tools.tool()
    }

    pub const fn tool_settings(&self) -> ToolSettings {
        self.tools.settings()
    }

    pub const fn selection(&self) -> Option<PixelRect> {
        self.tools.selection()
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    pub fn undo_len(&self) -> usize {
        self.history.undo_len()
    }

    pub fn redo_len(&self) -> usize {
        self.history.redo_len()
    }

    pub fn project_path(&self) -> Option<&Path> {
        self.project_path.as_deref()
    }

    /// `true` when the document differs from the last saved/opened state.
    pub fn is_modified(&self) -> bool {
        self.current_state() != self.saved_state
    }

    // ----- tool settings -----

    pub fn select_tool(&mut self, tool: ToolKind) {
        let events = self.tools.set_tool(tool);
        // Switching tools only cancels previews; it never commits.
        let _ = self.apply_tool_events(events);
    }

    pub fn set_color(&mut self, color: Color) {
        self.tools.settings_mut().color = color;
    }

    pub fn set_pen_width(&mut self, width: StrokeWidth) {
        self.tools.settings_mut().pen_width = width;
    }

    pub fn set_eraser_width(&mut self, width: StrokeWidth) {
        self.tools.settings_mut().eraser_width = width;
    }

    // ----- pointer / keyboard input (canvas coordinates) -----

    /// Shift modifier state (straight-line segment while drawing).
    pub fn set_shift(&mut self, shift: bool) {
        self.tools.set_shift(shift);
    }

    pub fn pointer_pressed(&mut self, p: Point) -> Result<(), AppError> {
        let events = self.tools.press(p, self.canvas());
        self.apply_tool_events(events)
    }

    pub fn pointer_dragged(&mut self, p: Point) -> Result<(), AppError> {
        let events = self.tools.drag(p, self.canvas());
        self.apply_tool_events(events)
    }

    pub fn pointer_released(&mut self, p: Point) -> Result<(), AppError> {
        let events = self.tools.release(p, self.canvas());
        self.apply_tool_events(events)
    }

    pub fn cancel_gesture(&mut self) {
        let events = self.tools.cancel();
        let _ = self.apply_tool_events(events);
    }

    /// Delete key: erase the selected rectangle on the drawing layer.
    pub fn delete_selection(&mut self) -> Result<(), AppError> {
        match self.tools.delete_selection() {
            Some(event) => self.apply_tool_events(vec![event]),
            None => Ok(()),
        }
    }

    /// Esc key.
    pub fn clear_selection(&mut self) {
        let _ = self.tools.clear_selection();
    }

    // ----- layers -----

    pub fn set_layer_visible(&mut self, id: LayerId, visible: bool) -> Result<(), AppError> {
        Ok(self.document.set_layer_visible(id, visible)?)
    }

    // ----- undo / redo -----

    pub fn undo(&mut self) -> Result<bool, AppError> {
        self.cancel_gesture();
        let Some(command) = self.history.undo(&mut self.document)?.cloned() else {
            return Ok(false);
        };
        if let Some(state) = self.undo_states.pop() {
            self.redo_states.push(state);
        }
        match command {
            EditCommand::AddOp { layer, .. } => self.rebuild_cache(layer),
            EditCommand::ReplaceUnderlay { .. } => self.rebuild_all_caches(),
        }
        Ok(true)
    }

    pub fn redo(&mut self) -> Result<bool, AppError> {
        self.cancel_gesture();
        let Some(command) = self.history.redo(&mut self.document)?.cloned() else {
            return Ok(false);
        };
        if let Some(state) = self.redo_states.pop() {
            self.undo_states.push(state);
        }
        match command {
            EditCommand::AddOp { layer, op } => {
                if let Some(cache) = self.caches.get_mut(&layer) {
                    cache.commit(&op);
                }
            }
            EditCommand::ReplaceUnderlay { .. } => self.rebuild_all_caches(),
        }
        Ok(true)
    }

    // ----- files -----

    /// Loads PNG/JPEG/BMP as the underlay. The canvas takes the image size;
    /// drawings are kept (top-left anchored). Undoable.
    pub fn import_image(&mut self, path: &Path) -> Result<(), AppError> {
        self.cancel_gesture();
        let image = self.services.image_importer.import(path)?;
        let after = Underlay {
            canvas: image.size(),
            image: Some(Arc::new(image)),
        };
        let before = self.document.underlay();
        self.execute(EditCommand::ReplaceUnderlay { before, after })?;
        self.tools.clear_selection();
        self.rebuild_all_caches();
        Ok(())
    }

    /// Saves to `path` and remembers it as the project path.
    pub fn save_project_as(&mut self, path: &Path) -> Result<(), AppError> {
        self.cancel_gesture();
        self.services.project_store.save(path, &self.document)?;
        self.project_path = Some(path.to_owned());
        self.saved_state = self.current_state();
        Ok(())
    }

    /// Saves to the current project path. Returns `Ok(false)` when there
    /// is none yet (the UI then asks for a path).
    pub fn save_project(&mut self) -> Result<bool, AppError> {
        match self.project_path.clone() {
            Some(path) => self.save_project_as(&path).map(|()| true),
            None => Ok(false),
        }
    }

    /// Opens a project. On failure the current document is unchanged.
    pub fn open_project(&mut self, path: &Path) -> Result<(), AppError> {
        let document = self.services.project_store.load(path)?;
        self.cancel_gesture();
        self.tools.clear_selection();
        self.document = document;
        self.history.clear();
        self.undo_states.clear();
        self.redo_states.clear();
        self.saved_state = 0;
        self.project_path = Some(path.to_owned());
        self.rebuild_all_caches();
        Ok(())
    }

    /// Writes a PNG (alpha channel preserved).
    pub fn export_png(&self, path: &Path, mode: ExportMode) -> Result<(), AppError> {
        let image = self.render_export(mode);
        self.services.image_exporter.export_png(path, &image)?;
        Ok(())
    }

    /// The image [`Self::export_png`] would write.
    pub fn render_export(&self, mode: ExportMode) -> paint_domain::RasterImage {
        compose_visible(&self.document, self.render_options, |layer| match mode {
            ExportMode::VisibleLayers => true,
            ExportMode::DrawingOnly => matches!(layer.content, LayerContent::Vector(_)),
        })
    }

    // ----- internals -----

    fn current_state(&self) -> u64 {
        self.undo_states.last().copied().unwrap_or(0)
    }

    fn execute(&mut self, command: EditCommand) -> Result<(), AppError> {
        self.history.execute(&mut self.document, command)?;
        self.undo_states.push(self.next_state);
        self.next_state += 1;
        self.redo_states.clear();
        Ok(())
    }

    fn apply_tool_events(&mut self, events: Vec<ToolEvent>) -> Result<(), AppError> {
        let layer = Document::DRAWING_LAYER;
        for event in events {
            match event {
                ToolEvent::Preview(op) => {
                    if let Some(cache) = self.caches.get_mut(&layer) {
                        cache.set_preview(&op);
                    }
                }
                ToolEvent::CancelPreview => {
                    if let Some(cache) = self.caches.get_mut(&layer) {
                        cache.clear_preview();
                    }
                }
                ToolEvent::Commit(op) => self.commit(layer, op)?,
                ToolEvent::Selection(_) => {}
            }
        }
        Ok(())
    }

    fn commit(&mut self, layer: LayerId, op: DrawOp) -> Result<(), AppError> {
        let result = self.execute(EditCommand::AddOp {
            layer,
            op: op.clone(),
        });
        if let Some(cache) = self.caches.get_mut(&layer) {
            match result {
                Ok(()) => cache.commit(&op),
                Err(_) => cache.clear_preview(),
            }
        }
        result
    }

    fn rebuild_cache(&mut self, id: LayerId) {
        let canvas = self.document.canvas();
        if let (Some(cache), Some(ops)) = (self.caches.get_mut(&id), self.document.ops(id)) {
            cache.rebuild(canvas, ops);
        }
    }

    fn rebuild_all_caches(&mut self) {
        let canvas = self.document.canvas();
        let options = self.render_options;
        self.caches = self
            .document
            .layers()
            .iter()
            .filter_map(|layer| {
                let ops = layer.ops()?;
                let mut cache = LayerCache::new(canvas, options);
                cache.rebuild(canvas, ops);
                Some((layer.id, cache))
            })
            .collect();
        self.underlay_revision += 1;
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use paint_domain::RasterImage;
    use paint_domain::ports::{ImageExporter, ImageImporter, ProjectStore};

    use super::*;

    /// In-memory adapters standing in for the file system.
    #[derive(Debug, Default)]
    struct Memory {
        projects: Mutex<HashMap<PathBuf, Document>>,
        images: Mutex<HashMap<PathBuf, RasterImage>>,
    }

    #[derive(Debug, Clone, Default)]
    struct Fake(Arc<Memory>);

    impl ProjectStore for Fake {
        fn save(&self, path: &Path, document: &Document) -> Result<(), PersistenceError> {
            if path.to_string_lossy().contains("readonly") {
                return Err(PersistenceError::Io("denied".to_owned()));
            }
            self.0
                .projects
                .lock()
                .unwrap()
                .insert(path.into(), document.clone());
            Ok(())
        }
        fn load(&self, path: &Path) -> Result<Document, PersistenceError> {
            self.0
                .projects
                .lock()
                .unwrap()
                .get(path)
                .cloned()
                .ok_or_else(|| PersistenceError::Io("not found".to_owned()))
        }
    }

    impl ImageImporter for Fake {
        fn import(&self, path: &Path) -> Result<RasterImage, PersistenceError> {
            self.0
                .images
                .lock()
                .unwrap()
                .get(path)
                .cloned()
                .ok_or_else(|| PersistenceError::Io("not found".to_owned()))
        }
    }

    impl ImageExporter for Fake {
        fn export_png(&self, path: &Path, image: &RasterImage) -> Result<(), PersistenceError> {
            self.0
                .images
                .lock()
                .unwrap()
                .insert(path.into(), image.clone());
            Ok(())
        }
    }

    const CANVAS: CanvasSize = CanvasSize {
        width: 64,
        height: 48,
    };
    const RED: Color = Color::rgb(255, 0, 0);

    fn vm() -> (PaintViewModel, Fake) {
        let fake = Fake::default();
        let services = Services {
            project_store: Box::new(fake.clone()),
            image_importer: Box::new(fake.clone()),
            image_exporter: Box::new(fake.clone()),
        };
        let mut vm = PaintViewModel::with_document(services, Document::new(CANVAS));
        vm.set_color(RED);
        (vm, fake)
    }

    fn pixel(vm: &PaintViewModel, x: u32, y: u32) -> Color {
        vm.layer_cache(Document::DRAWING_LAYER)
            .unwrap()
            .raster()
            .pixel(x, y)
            .unwrap()
    }

    fn line(vm: &mut PaintViewModel, from: (f32, f32), to: (f32, f32)) {
        vm.pointer_pressed(Point::new(from.0, from.1)).unwrap();
        vm.pointer_dragged(Point::new(to.0, to.1)).unwrap();
        vm.pointer_released(Point::new(to.0, to.1)).unwrap();
    }

    fn ops(vm: &PaintViewModel) -> usize {
        vm.document().ops(Document::DRAWING_LAYER).unwrap().len()
    }

    #[test]
    fn shift_drag_commits_one_straight_segment() {
        let (mut vm, _) = vm();
        vm.pointer_pressed(Point::new(0.0, 10.0)).unwrap();
        vm.set_shift(true);
        vm.pointer_dragged(Point::new(20.0, 30.0)).unwrap();
        vm.pointer_released(Point::new(40.0, 10.0)).unwrap();
        vm.set_shift(false);
        assert_eq!(vm.undo_len(), 1);
        let img = vm.render_export(ExportMode::DrawingOnly);
        assert!(img.pixel(20, 10).unwrap().a > 0, "on the straight line");
        assert_eq!(img.pixel(20, 30).unwrap().a, 0, "detour not drawn");
    }

    #[test]
    fn drawing_previews_then_commits() {
        let (mut vm, _) = vm();
        vm.pointer_pressed(Point::new(10.5, 10.5)).unwrap();
        vm.pointer_dragged(Point::new(30.5, 10.5)).unwrap();
        assert_eq!(pixel(&vm, 20, 10), RED, "preview is visible");
        assert_eq!(ops(&vm), 0, "preview is not committed");
        vm.pointer_released(Point::new(30.5, 10.5)).unwrap();
        assert_eq!(ops(&vm), 1);
        assert!(vm.can_undo());
        assert!(vm.is_modified());
        assert!(vm.take_layer_dirty(Document::DRAWING_LAYER).is_some());
        assert!(vm.take_layer_dirty(Document::DRAWING_LAYER).is_none());
    }

    #[test]
    fn cancel_removes_preview() {
        let (mut vm, _) = vm();
        vm.pointer_pressed(Point::new(10.5, 10.5)).unwrap();
        vm.cancel_gesture();
        assert_eq!(pixel(&vm, 10, 10), Color::TRANSPARENT);
        assert_eq!(ops(&vm), 0);
    }

    #[test]
    fn undo_redo_updates_document_and_display() {
        let (mut vm, _) = vm();
        line(&mut vm, (5.5, 5.5), (20.5, 5.5));
        vm.select_tool(ToolKind::Eraser);
        vm.set_eraser_width(StrokeWidth::Px8);
        line(&mut vm, (12.5, 5.5), (12.5, 5.5));
        assert_eq!(pixel(&vm, 12, 5), Color::TRANSPARENT);

        assert!(vm.undo().unwrap());
        assert_eq!(pixel(&vm, 12, 5), RED);
        assert!(vm.undo().unwrap());
        assert_eq!(pixel(&vm, 6, 5), Color::TRANSPARENT);
        assert!(!vm.undo().unwrap());
        assert!(!vm.is_modified());

        assert!(vm.redo().unwrap());
        assert_eq!(pixel(&vm, 12, 5), RED);
        assert!(vm.redo().unwrap());
        assert_eq!(pixel(&vm, 12, 5), Color::TRANSPARENT);
        assert!(!vm.redo().unwrap());
    }

    #[test]
    fn supports_more_than_fifty_undo_and_redo() {
        let (mut vm, _) = vm();
        for i in 0..60 {
            let x = i as f32 + 0.5;
            line(&mut vm, (x, 1.5), (x, 1.5));
        }
        for _ in 0..60 {
            assert!(vm.undo().unwrap());
        }
        assert_eq!(ops(&vm), 0);
        for _ in 0..60 {
            assert!(vm.redo().unwrap());
        }
        assert_eq!(ops(&vm), 60);
        assert_eq!(pixel(&vm, 59, 1), RED);
    }

    #[test]
    fn new_edit_after_undo_clears_redo() {
        let (mut vm, _) = vm();
        line(&mut vm, (1.5, 1.5), (5.5, 1.5));
        vm.undo().unwrap();
        line(&mut vm, (1.5, 9.5), (5.5, 9.5));
        assert!(!vm.can_redo());
    }

    #[test]
    fn range_erase_with_delete_is_undoable() {
        let (mut vm, _) = vm();
        vm.set_pen_width(StrokeWidth::Px16);
        line(&mut vm, (0.0, 24.0), (64.0, 24.0));
        vm.select_tool(ToolKind::Select);
        line(&mut vm, (10.0, 0.0), (30.0, 48.0));
        assert_eq!(vm.selection(), Some(PixelRect::new(10, 0, 20, 48)));
        vm.delete_selection().unwrap();
        assert_eq!(pixel(&vm, 20, 24), Color::TRANSPARENT);
        assert_eq!(pixel(&vm, 5, 24), RED);
        vm.undo().unwrap();
        assert_eq!(pixel(&vm, 20, 24), RED);
        vm.clear_selection();
        vm.delete_selection().unwrap();
        assert_eq!(ops(&vm), 1, "Delete without selection does nothing");
    }

    #[test]
    fn import_image_sets_underlay_and_canvas_and_is_undoable() {
        let (mut vm, fake) = vm();
        line(&mut vm, (1.5, 1.5), (5.5, 1.5));
        let size = CanvasSize::new(100, 30).unwrap();
        fake.0
            .images
            .lock()
            .unwrap()
            .insert("bg.png".into(), RasterImage::filled(size, Color::WHITE));
        let revision = vm.underlay_revision();
        vm.import_image(Path::new("bg.png")).unwrap();
        assert_eq!(vm.canvas(), size);
        assert!(vm.underlay_revision() > revision);
        assert!(vm.underlay_image().is_some());
        assert_eq!(pixel(&vm, 3, 1), RED, "drawing is kept");
        vm.undo().unwrap();
        assert_eq!(vm.canvas(), CANVAS);
        assert!(vm.underlay_image().is_none());
        vm.redo().unwrap();
        assert_eq!(vm.canvas(), size);
    }

    #[test]
    fn failed_import_keeps_document() {
        let (mut vm, _) = vm();
        assert!(vm.import_image(Path::new("missing.png")).is_err());
        assert_eq!(vm.canvas(), CANVAS);
        assert!(!vm.can_undo());
    }

    #[test]
    fn save_and_open_round_trip() {
        let (mut vm, _) = vm();
        assert!(!vm.save_project().unwrap(), "no path yet");
        line(&mut vm, (1.5, 1.5), (5.5, 1.5));
        vm.save_project_as(Path::new("a.paint")).unwrap();
        assert!(!vm.is_modified());
        assert_eq!(vm.project_path(), Some(Path::new("a.paint")));
        let saved = vm.document().clone();

        line(&mut vm, (1.5, 9.5), (5.5, 9.5));
        assert!(vm.is_modified());
        vm.open_project(Path::new("a.paint")).unwrap();
        assert_eq!(vm.document(), &saved);
        assert!(!vm.is_modified());
        assert!(!vm.can_undo(), "history is reset on open");
        assert_eq!(pixel(&vm, 3, 1), RED);
        assert_eq!(pixel(&vm, 3, 9), Color::TRANSPARENT);
    }

    #[test]
    fn modified_flag_follows_undo_to_saved_state() {
        let (mut vm, _) = vm();
        line(&mut vm, (1.5, 1.5), (5.5, 1.5));
        vm.save_project_as(Path::new("a.paint")).unwrap();
        line(&mut vm, (1.5, 9.5), (5.5, 9.5));
        vm.undo().unwrap();
        assert!(!vm.is_modified());
        vm.undo().unwrap();
        assert!(vm.is_modified());
        vm.redo().unwrap();
        assert!(!vm.is_modified());
        assert!(vm.save_project().unwrap());
    }

    #[test]
    fn failed_save_and_open_keep_state() {
        let (mut vm, _) = vm();
        line(&mut vm, (1.5, 1.5), (5.5, 1.5));
        assert!(vm.save_project_as(Path::new("readonly/a.paint")).is_err());
        assert!(vm.is_modified());
        assert_eq!(vm.project_path(), None);
        let before = vm.document().clone();
        assert!(vm.open_project(Path::new("missing.paint")).is_err());
        assert_eq!(vm.document(), &before);
        assert!(vm.can_undo());
    }

    #[test]
    fn export_modes() {
        let (mut vm, fake) = vm();
        let mut bg = RasterImage::filled(CANVAS, Color::WHITE);
        bg.set_pixel(0, 0, Color::BLACK);
        fake.0.images.lock().unwrap().insert("bg.png".into(), bg);
        vm.import_image(Path::new("bg.png")).unwrap();
        line(&mut vm, (10.5, 10.5), (10.5, 10.5));

        let all = vm.render_export(ExportMode::VisibleLayers);
        assert_eq!(all.pixel(0, 0), Some(Color::BLACK));
        assert_eq!(all.pixel(10, 10), Some(RED));
        let drawing = vm.render_export(ExportMode::DrawingOnly);
        assert_eq!(drawing.pixel(0, 0), Some(Color::TRANSPARENT));
        assert_eq!(drawing.pixel(10, 10), Some(RED));

        vm.set_layer_visible(Document::UNDERLAY_LAYER, false)
            .unwrap();
        assert_eq!(
            vm.render_export(ExportMode::VisibleLayers).pixel(0, 0),
            Some(Color::TRANSPARENT)
        );

        vm.export_png(Path::new("out.png"), ExportMode::DrawingOnly)
            .unwrap();
        assert_eq!(
            fake.0.images.lock().unwrap()["out.png".as_ref() as &Path],
            drawing
        );
    }

    #[test]
    fn tool_settings_are_exposed() {
        let (mut vm, _) = vm();
        vm.select_tool(ToolKind::Eraser);
        vm.set_pen_width(StrokeWidth::Px32);
        assert_eq!(vm.tool(), ToolKind::Eraser);
        assert_eq!(vm.tool_settings().pen_width, StrokeWidth::Px32);
        assert_eq!(vm.tool_settings().color, RED);
    }

    #[test]
    fn errors_are_displayable() {
        let e = AppError::from(PersistenceError::Io("x".to_owned()));
        assert!(e.to_string().contains('x'));
        assert!(
            !AppError::from(DomainError::NoUnderlayLayer)
                .to_string()
                .is_empty()
        );
    }
}
