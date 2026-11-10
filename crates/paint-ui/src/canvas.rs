use eframe::egui::{
    self, Color32, ColorImage, Pos2, Rect, Sense, TextureFilter, TextureHandle, TextureOptions,
    Vec2,
};
use paint_app::PaintViewModel;
use paint_domain::{Document, Point, RasterImage};
use paint_engine::extract_region;

pub(crate) const MIN_ZOOM: f32 = 0.1;
pub(crate) const MAX_ZOOM: f32 = 32.0;

const TEXTURE_OPTIONS: TextureOptions = TextureOptions {
    magnification: TextureFilter::Nearest,
    minification: TextureFilter::Linear,
    ..TextureOptions::NEAREST
};

/// Canvas view state: zoom, scroll, GPU textures and the pointer gesture.
#[derive(Default)]
pub(crate) struct CanvasView {
    zoom: f32,
    pending_offset: Option<Vec2>,
    underlay: Option<(u64, TextureHandle)>,
    drawing: Option<TextureHandle>,
    drawing_active: bool,
    panning: bool,
    pub(crate) cursor: Option<Point>,
    pub(crate) screen_rect: Option<Rect>,
}

impl std::fmt::Debug for CanvasView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CanvasView")
            .field("zoom", &self.zoom)
            .field("cursor", &self.cursor)
            .finish_non_exhaustive()
    }
}

impl CanvasView {
    pub(crate) fn new() -> Self {
        Self {
            zoom: 1.0,
            ..Default::default()
        }
    }

    pub(crate) fn zoom(&self) -> f32 {
        self.zoom
    }

    pub(crate) fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    }

    /// Converts a screen position to canvas pixel coordinates.
    pub(crate) fn to_canvas(&self, pos: Pos2) -> Option<Point> {
        let rect = self.screen_rect?;
        let p = (pos - rect.min) / self.zoom;
        Some(Point::new(p.x, p.y))
    }

    /// Screen position of a canvas point (used by E2E tests).
    pub fn to_screen(&self, p: Point) -> Option<Pos2> {
        let rect = self.screen_rect?;
        Some(rect.min + Vec2::new(p.x, p.y) * self.zoom)
    }

    pub(crate) fn show(
        &mut self,
        ui: &mut egui::Ui,
        vm: &mut PaintViewModel,
    ) -> Result<(), String> {
        self.sync_textures(ui.ctx(), vm);
        let canvas = vm.canvas();
        let size = Vec2::new(canvas.width as f32, canvas.height as f32) * self.zoom;

        let mut area = egui::ScrollArea::both().auto_shrink(false);
        if let Some(offset) = self.pending_offset.take() {
            area = area.scroll_offset(offset);
        }
        let mut result = Ok(());
        let output = area.show(ui, |ui| {
            let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
            self.screen_rect = Some(rect);
            self.paint(ui, rect, vm);
            result = self.handle_pointer(ui, &response, vm);
            response.hovered()
        });
        let offset = output.state.clamped_offset();
        if output.inner {
            self.handle_zoom(ui, offset, output.inner_rect);
        }
        self.handle_pan(ui, output.inner, offset);
        result
    }

    fn paint(&self, ui: &egui::Ui, rect: Rect, vm: &PaintViewModel) {
        let painter = ui.painter_at(rect);
        paint_checkerboard(&painter, rect, self.zoom);
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        let doc = vm.document();
        let visible = |id| doc.layer(id).is_some_and(|l| l.visible);
        if visible(Document::UNDERLAY_LAYER)
            && let Some((_, tex)) = &self.underlay
        {
            painter.image(tex.id(), rect, uv, Color32::WHITE);
        }
        if visible(Document::DRAWING_LAYER)
            && let Some(tex) = &self.drawing
        {
            painter.image(tex.id(), rect, uv, Color32::WHITE);
        }
        if let Some(sel) = vm.selection() {
            let min = rect.min + Vec2::new(sel.x as f32, sel.y as f32) * self.zoom;
            let r = Rect::from_min_size(
                min,
                Vec2::new(sel.width as f32, sel.height as f32) * self.zoom,
            );
            painter.rect_stroke(
                r,
                0.0,
                egui::Stroke::new(1.0, Color32::from_rgb(0, 120, 215)),
                egui::StrokeKind::Middle,
            );
        }
    }

    fn handle_pointer(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        vm: &mut PaintViewModel,
    ) -> Result<(), String> {
        let (pos, pressed, down, released, shift) = ui.input(|i| {
            (
                i.pointer.latest_pos(),
                i.pointer.primary_pressed(),
                i.pointer.primary_down(),
                i.pointer.primary_released(),
                i.modifiers.shift,
            )
        });
        vm.set_shift(shift);
        self.cursor = pos
            .filter(|_| response.hovered() || self.drawing_active)
            .and_then(|p| self.to_canvas(p));
        let Some(p) = pos.and_then(|p| self.to_canvas(p)) else {
            return Ok(());
        };
        let result = if pressed && response.hovered() {
            self.drawing_active = true;
            vm.pointer_pressed(p)
        } else if self.drawing_active && released {
            self.drawing_active = false;
            vm.pointer_released(p)
        } else if self.drawing_active && down {
            vm.pointer_dragged(p)
        } else {
            Ok(())
        };
        result.map_err(|e| e.to_string())
    }

    /// Middle-button drag pans the view.
    fn handle_pan(&mut self, ui: &egui::Ui, hovered: bool, offset: Vec2) {
        let (down, delta) = ui.input(|i| (i.pointer.middle_down(), i.pointer.delta()));
        self.panning = down && (self.panning || hovered);
        if self.panning && delta != Vec2::ZERO && self.pending_offset.is_none() {
            self.pending_offset = Some(offset - delta);
        }
    }

    /// Ctrl + mouse wheel zoom, keeping the canvas point under the cursor.
    fn handle_zoom(&mut self, ui: &egui::Ui, offset: Vec2, view: Rect) {
        let (factor, pointer) = ui.input(|i| (i.zoom_delta(), i.pointer.hover_pos()));
        if (factor - 1.0).abs() < f32::EPSILON {
            return;
        }
        let old = self.zoom;
        self.set_zoom(old * factor);
        if let Some(pointer) = pointer {
            let in_view = pointer - view.min;
            self.pending_offset = Some((offset + in_view) * (self.zoom / old) - in_view);
        }
    }

    fn sync_textures(&mut self, ctx: &egui::Context, vm: &mut PaintViewModel) {
        let revision = vm.underlay_revision();
        if self.underlay.as_ref().is_none_or(|(r, _)| *r != revision) {
            let image = vm
                .underlay_image()
                .map(|img| to_color_image(img))
                .unwrap_or_else(|| ColorImage::filled([1, 1], Color32::TRANSPARENT));
            let tex = ctx.load_texture("underlay", image, TEXTURE_OPTIONS);
            self.underlay = Some((revision, tex));
        }

        let dirty = vm.take_layer_dirty(Document::DRAWING_LAYER);
        let Some(cache) = vm.layer_cache(Document::DRAWING_LAYER) else {
            return;
        };
        let raster = cache.raster();
        let size = [raster.width() as usize, raster.height() as usize];
        match &mut self.drawing {
            Some(tex) if tex.size() == size => {
                if let Some(rect) = dirty {
                    let data = extract_region(raster, rect);
                    let region = ColorImage::from_rgba_unmultiplied(
                        [rect.width as usize, rect.height as usize],
                        &data,
                    );
                    tex.set_partial([rect.x as usize, rect.y as usize], region, TEXTURE_OPTIONS);
                }
            }
            _ => {
                self.drawing =
                    Some(ctx.load_texture("drawing", to_color_image(raster), TEXTURE_OPTIONS));
            }
        }
    }
}

pub(crate) fn to_color_image(image: &RasterImage) -> ColorImage {
    ColorImage::from_rgba_unmultiplied(
        [image.width() as usize, image.height() as usize],
        image.pixels(),
    )
}

/// Transparency checkerboard behind the layers.
fn paint_checkerboard(painter: &egui::Painter, rect: Rect, zoom: f32) {
    painter.rect_filled(rect, 0.0, Color32::from_gray(255));
    let cell = (8.0 * zoom.max(1.0)).min(32.0);
    let clip = painter.clip_rect().intersect(rect);
    let start_x = ((clip.min.x - rect.min.x) / cell).floor() as i32;
    let start_y = ((clip.min.y - rect.min.y) / cell).floor() as i32;
    let end_x = ((clip.max.x - rect.min.x) / cell).ceil() as i32;
    let end_y = ((clip.max.y - rect.min.y) / cell).ceil() as i32;
    for y in start_y..end_y {
        for x in start_x..end_x {
            if (x + y) % 2 == 0 {
                continue;
            }
            let min = rect.min + Vec2::new(x as f32, y as f32) * cell;
            let r = Rect::from_min_size(min, Vec2::splat(cell)).intersect(rect);
            painter.rect_filled(r, 0.0, Color32::from_gray(220));
        }
    }
}
