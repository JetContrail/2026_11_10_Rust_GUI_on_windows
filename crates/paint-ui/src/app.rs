use crate::i18n::tr;
use eframe::egui::{self, Color32, Key, KeyboardShortcut, Modifiers};
use paint_app::{ExportMode, PaintViewModel};
use paint_domain::{Color, PALETTE, StrokeWidth};
use paint_tools::ToolKind;

use crate::FileDialogs;
use crate::canvas::CanvasView;
use crate::text_ui::{FontPicker, TextInput, text_settings};

pub const APP_TITLE: &str = "Rust Paint";

const OPEN_IMAGE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
const SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
const UNDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
const REDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);

/// An action that discards unsaved changes and therefore needs confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingAction {
    OpenProject,
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Choice {
    Save,
    Discard,
    Cancel,
}

/// Main window: tool palette (left), canvas (center), layer panel (right),
/// status bar (bottom).
#[derive(Debug)]
pub struct PaintApp {
    vm: PaintViewModel,
    dialogs: Box<dyn FileDialogs>,
    canvas: CanvasView,
    message: String,
    title: String,
    confirm: Option<PendingAction>,
    allow_close: bool,
    text_input: TextInput,
    font_picker: Option<FontPicker>,
}

impl PaintApp {
    pub fn new(vm: PaintViewModel, dialogs: Box<dyn FileDialogs>) -> Self {
        Self {
            vm,
            dialogs,
            canvas: CanvasView::new(),
            message: tr().ready.to_owned(),
            title: String::new(),
            confirm: None,
            allow_close: false,
            text_input: TextInput::default(),
            font_picker: None,
        }
    }

    pub fn view_model(&self) -> &PaintViewModel {
        &self.vm
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn zoom(&self) -> f32 {
        self.canvas.zoom()
    }

    /// Screen position of a canvas point (for E2E tests).
    pub fn canvas_to_screen(&self, x: f32, y: f32) -> Option<egui::Pos2> {
        self.canvas.to_screen(paint_domain::Point::new(x, y))
    }

    /// `true` while the unsaved-changes confirmation dialog is shown.
    pub fn is_confirming(&self) -> bool {
        self.confirm.is_some()
    }

    /// Draws one frame.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        self.handle_close_request(ui.ctx());
        self.confirm_dialog(ui.ctx());
        if self.confirm.is_none() {
            self.handle_shortcuts(ui.ctx());
        }
        self.update_title(ui.ctx());
        egui::Panel::top("menu").show(ui, |ui| self.menu_bar(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::Panel::left("tools")
            .resizable(false)
            .exact_size(200.0)
            .show(ui, |ui| self.tool_palette(ui));
        egui::Panel::right("layers")
            .resizable(false)
            .exact_size(180.0)
            .show(ui, |ui| self.layer_panel(ui));
        egui::CentralPanel::default().show(ui, |ui| {
            if let Err(e) = self.canvas.show(ui, &mut self.vm) {
                self.message = e;
            }
        });
        if let Err(e) = self.text_input.show(ui.ctx(), &mut self.vm, &self.canvas) {
            self.message = e;
        }
        if let Some(picker) = &mut self.font_picker
            && !picker.show(ui.ctx(), &mut self.vm)
        {
            self.font_picker = None;
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let (open, save, undo, redo, delete, esc) = ctx.input_mut(|i| {
            (
                i.consume_shortcut(&OPEN_IMAGE),
                i.consume_shortcut(&SAVE),
                i.consume_shortcut(&UNDO),
                i.consume_shortcut(&REDO),
                i.consume_key(Modifiers::NONE, Key::Delete),
                i.consume_key(Modifiers::NONE, Key::Escape),
            )
        });
        if open {
            self.open_image();
        }
        if save {
            self.save();
        }
        if undo {
            self.undo();
        }
        if redo {
            self.redo();
        }
        if delete {
            self.delete_selection();
        }
        if esc && !self.vm.cancel_text() {
            self.vm.clear_selection();
        }
    }

    fn handle_close_request(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            if self.vm.is_modified() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.confirm = Some(PendingAction::Exit);
            } else {
                self.allow_close = true;
            }
        }
    }

    /// Modal "save changes?" dialog shown before discarding unsaved work.
    fn confirm_dialog(&mut self, ctx: &egui::Context) {
        let Some(action) = self.confirm else {
            return;
        };
        let mut choice = None;
        let modal = egui::Modal::new(egui::Id::unique("unsaved-changes")).show(ctx, |ui| {
            ui.heading(tr().unsaved_title);
            ui.label(tr().unsaved_question);
            ui.horizontal(|ui| {
                for (label, c) in [
                    (tr().save, Choice::Save),
                    (tr().dont_save, Choice::Discard),
                    (tr().cancel, Choice::Cancel),
                ] {
                    if ui.button(label).clicked() {
                        choice = Some(c);
                    }
                }
            });
        });
        if modal.should_close() && choice.is_none() {
            choice = Some(Choice::Cancel);
        }
        let Some(choice) = choice else {
            return;
        };
        self.confirm = None;
        match choice {
            Choice::Cancel => {}
            Choice::Save => {
                self.save();
                if !self.vm.is_modified() {
                    self.run(ctx, action);
                }
            }
            Choice::Discard => self.run(ctx, action),
        }
    }

    fn run(&mut self, ctx: &egui::Context, action: PendingAction) {
        match action {
            PendingAction::OpenProject => self.open_project_unchecked(),
            PendingAction::Exit => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let name = self
            .vm
            .project_path()
            .and_then(|p| p.file_name())
            .map_or_else(
                || tr().untitled.to_owned(),
                |n| n.to_string_lossy().into_owned(),
            );
        let mark = if self.vm.is_modified() { " *" } else { "" };
        let title = format!("{name}{mark} - {APP_TITLE}");
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button(tr().menu_file, |ui| {
                if ui.button(tr().open_image).clicked() {
                    self.open_image();
                }
                if ui.button(tr().open_project).clicked() {
                    self.open_project();
                }
                ui.separator();
                if ui.button(tr().save_shortcut).clicked() {
                    self.save();
                }
                if ui.button(tr().save_as).clicked() {
                    self.save_as();
                }
                ui.separator();
                if ui.button(tr().export_png).clicked() {
                    self.export(ExportMode::VisibleLayers);
                }
                if ui.button(tr().export_transparent_png).clicked() {
                    self.export(ExportMode::DrawingOnly);
                }
                ui.separator();
                if ui.button(tr().exit).clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button(tr().menu_edit, |ui| {
                if ui
                    .add_enabled(self.vm.can_undo(), egui::Button::new(tr().undo_shortcut))
                    .clicked()
                {
                    self.undo();
                }
                if ui
                    .add_enabled(self.vm.can_redo(), egui::Button::new(tr().redo_shortcut))
                    .clicked()
                {
                    self.redo();
                }
                if ui.button(tr().delete_selection).clicked() {
                    self.delete_selection();
                }
            });
            ui.menu_button(tr().menu_view, |ui| {
                for (label, zoom) in [(tr().zoom_in, 2.0), (tr().zoom_out, 0.5)] {
                    if ui.button(label).clicked() {
                        self.canvas.set_zoom(self.canvas.zoom() * zoom);
                    }
                }
                if ui.button("100%").clicked() {
                    self.canvas.set_zoom(1.0);
                }
            });
        });
    }

    fn tool_palette(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr().tools);
        for tool in ToolKind::ALL {
            if ui
                .selectable_label(self.vm.tool() == tool, tool_name(tool))
                .clicked()
            {
                self.vm.select_tool(tool);
            }
        }
        if self.vm.tool() == ToolKind::Text {
            ui.separator();
            if text_settings(ui, &mut self.vm) {
                self.font_picker = Some(FontPicker::new(&self.vm));
            }
        }
        let settings = self.vm.tool_settings();
        ui.separator();
        ui.label(tr().pen_width);
        width_selector(ui, "pen_width", settings.pen_width, |w| {
            self.vm.set_pen_width(w)
        });
        ui.label(tr().eraser_size);
        width_selector(ui, "eraser_width", settings.eraser_width, |w| {
            self.vm.set_eraser_width(w);
        });

        ui.separator();
        ui.label(tr().color);
        let mut rgb = [settings.color.r, settings.color.g, settings.color.b];
        if ui.color_edit_button_srgb(&mut rgb).changed() {
            self.vm.set_color(Color::rgb(rgb[0], rgb[1], rgb[2]));
        }
        let mut changed = false;
        for (label, value) in ["R", "G", "B"].into_iter().zip(rgb.iter_mut()) {
            changed |= ui
                .add(egui::Slider::new(value, 0..=255).text(label))
                .changed();
        }
        if changed {
            self.vm.set_color(Color::rgb(rgb[0], rgb[1], rgb[2]));
        }
        egui::Grid::new("palette")
            .spacing([2.0, 2.0])
            .show(ui, |ui| {
                for (i, color) in PALETTE.iter().enumerate() {
                    let fill = Color32::from_rgb(color.r, color.g, color.b);
                    let selected = *color == settings.color;
                    let button = egui::Button::new("")
                        .fill(fill)
                        .min_size(egui::vec2(32.0, 20.0))
                        .stroke(egui::Stroke::new(
                            if selected { 2.0 } else { 0.5 },
                            Color32::GRAY,
                        ));
                    if ui.add(button).on_hover_text(color.to_hex()).clicked() {
                        self.vm.set_color(*color);
                    }
                    if i % 4 == 3 {
                        ui.end_row();
                    }
                }
            });
    }

    fn layer_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr().layers);
        let layers: Vec<_> = self
            .vm
            .document()
            .layers()
            .iter()
            .rev()
            .map(|l| (l.id, l.name.clone(), l.visible, l.locked))
            .collect();
        for (id, name, mut visible, locked) in layers {
            ui.horizontal(|ui| {
                if ui
                    .checkbox(&mut visible, "")
                    .on_hover_text(tr().layer_visible)
                    .changed()
                {
                    let _ = self.vm.set_layer_visible(id, visible);
                }
                let label = if locked {
                    format!("{name} {}", tr().layer_locked_suffix)
                } else {
                    format!("{name} {}", tr().layer_editing_suffix)
                };
                ui.label(label);
            });
        }
    }

    fn status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(tool_name(self.vm.tool()));
            ui.separator();
            let canvas = self.vm.canvas();
            ui.label(format!("{}×{} px", canvas.width, canvas.height));
            ui.separator();
            ui.label(format!("{:.0}%", self.canvas.zoom() * 100.0));
            ui.separator();
            match self.canvas.cursor {
                Some(p) => ui.label(format!("({:.0}, {:.0})", p.x.floor(), p.y.floor())),
                None => ui.label("(-, -)"),
            };
            ui.separator();
            ui.label(format!(
                "{} {} / {} {}",
                tr().undo_count,
                self.vm.undo_len(),
                tr().redo_count,
                self.vm.redo_len()
            ));
            ui.separator();
            ui.label(&self.message);
        });
    }

    // ----- commands -----

    fn open_image(&mut self) {
        if let Some(path) = self.dialogs.open_image() {
            let result = self.vm.import_image(&path);
            self.report(result, tr().image_loaded);
        }
    }

    fn open_project(&mut self) {
        if self.vm.is_modified() {
            self.confirm = Some(PendingAction::OpenProject);
        } else {
            self.open_project_unchecked();
        }
    }

    fn open_project_unchecked(&mut self) {
        if let Some(path) = self.dialogs.open_project() {
            let result = self.vm.open_project(&path);
            self.report(result, tr().project_opened);
        }
    }

    fn save(&mut self) {
        match self.vm.save_project() {
            Ok(true) => self.message = tr().saved.to_owned(),
            Ok(false) => self.save_as(),
            Err(e) => self.message = e.to_string(),
        }
    }

    fn save_as(&mut self) {
        if let Some(path) = self.dialogs.save_project() {
            let result = self.vm.save_project_as(&path);
            self.report(result, tr().saved);
        }
    }

    fn export(&mut self, mode: ExportMode) {
        if let Some(path) = self.dialogs.export_png() {
            let result = self.vm.export_png(&path, mode);
            self.report(result, tr().png_exported);
        }
    }

    fn delete_selection(&mut self) {
        let result = self.vm.delete_selection();
        self.report(result, tr().selection_deleted);
    }

    fn undo(&mut self) {
        let result = self.vm.undo();
        self.report(result.map(drop), tr().undone);
    }

    fn redo(&mut self) {
        let result = self.vm.redo();
        self.report(result.map(drop), tr().redone);
    }

    fn report(&mut self, result: Result<(), paint_app::AppError>, ok: &str) {
        self.message = match result {
            Ok(()) => ok.to_owned(),
            Err(e) => e.to_string(),
        };
    }
}

impl eframe::App for PaintApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }
}

fn tool_name(tool: ToolKind) -> &'static str {
    match tool {
        ToolKind::Pen => tr().tool_pen,
        ToolKind::Line => tr().tool_line,
        ToolKind::Eraser => tr().tool_eraser,
        ToolKind::Select => tr().tool_select,
        ToolKind::Text => tr().tool_text,
    }
}

fn width_selector(
    ui: &mut egui::Ui,
    id: &str,
    current: StrokeWidth,
    mut select: impl FnMut(StrokeWidth),
) {
    ui.push_id(id, |ui| {
        ui.horizontal_wrapped(|ui| {
            for width in StrokeWidth::ALL {
                if ui
                    .selectable_label(current == width, format!("{}", width.px()))
                    .clicked()
                {
                    select(width);
                }
            }
        });
    });
}
