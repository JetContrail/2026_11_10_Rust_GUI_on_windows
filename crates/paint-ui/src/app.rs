use eframe::egui::{self, Color32, Key, KeyboardShortcut, Modifiers};
use paint_app::{ExportMode, PaintViewModel};
use paint_domain::{Color, PALETTE, StrokeWidth};
use paint_tools::ToolKind;

use crate::FileDialogs;
use crate::canvas::CanvasView;

pub const APP_TITLE: &str = "Rust Paint";

const OPEN_IMAGE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
const SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
const UNDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
const REDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);

/// Main window: tool palette (left), canvas (center), layer panel (right),
/// status bar (bottom).
#[derive(Debug)]
pub struct PaintApp {
    vm: PaintViewModel,
    dialogs: Box<dyn FileDialogs>,
    canvas: CanvasView,
    message: String,
    title: String,
}

impl PaintApp {
    pub fn new(vm: PaintViewModel, dialogs: Box<dyn FileDialogs>) -> Self {
        Self {
            vm,
            dialogs,
            canvas: CanvasView::new(),
            message: "準備完了".to_owned(),
            title: String::new(),
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

    /// Draws one frame.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        self.handle_shortcuts(ui.ctx());
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
        if esc {
            self.vm.clear_selection();
        }
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let name = self
            .vm
            .project_path()
            .and_then(|p| p.file_name())
            .map_or_else(|| "無題".to_owned(), |n| n.to_string_lossy().into_owned());
        let mark = if self.vm.is_modified() { " *" } else { "" };
        let title = format!("{name}{mark} - {APP_TITLE}");
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("ファイル", |ui| {
                if ui.button("画像を開く… (Ctrl+O)").clicked() {
                    self.open_image();
                }
                if ui.button("プロジェクトを開く…").clicked() {
                    self.open_project();
                }
                ui.separator();
                if ui.button("保存 (Ctrl+S)").clicked() {
                    self.save();
                }
                if ui.button("名前を付けて保存…").clicked() {
                    self.save_as();
                }
                ui.separator();
                if ui.button("PNG 書き出し (表示中レイヤー)…").clicked() {
                    self.export(ExportMode::VisibleLayers);
                }
                if ui.button("透過 PNG 書き出し (描画レイヤーのみ)…").clicked() {
                    self.export(ExportMode::DrawingOnly);
                }
            });
            ui.menu_button("編集", |ui| {
                if ui
                    .add_enabled(self.vm.can_undo(), egui::Button::new("元に戻す (Ctrl+Z)"))
                    .clicked()
                {
                    self.undo();
                }
                if ui
                    .add_enabled(self.vm.can_redo(), egui::Button::new("やり直し (Ctrl+Y)"))
                    .clicked()
                {
                    self.redo();
                }
                if ui.button("選択範囲を削除 (Delete)").clicked() {
                    self.delete_selection();
                }
            });
            ui.menu_button("表示", |ui| {
                for (label, zoom) in [("拡大", 2.0), ("縮小", 0.5)] {
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
        ui.heading("ツール");
        for tool in ToolKind::ALL {
            if ui
                .selectable_label(self.vm.tool() == tool, tool_name(tool))
                .clicked()
            {
                self.vm.select_tool(tool);
            }
        }
        let settings = self.vm.tool_settings();
        ui.separator();
        ui.label("線幅");
        width_selector(ui, "pen_width", settings.pen_width, |w| {
            self.vm.set_pen_width(w)
        });
        ui.label("消しゴムサイズ");
        width_selector(ui, "eraser_width", settings.eraser_width, |w| {
            self.vm.set_eraser_width(w);
        });

        ui.separator();
        ui.label("色");
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
        ui.heading("レイヤー");
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
                    .on_hover_text("表示")
                    .changed()
                {
                    let _ = self.vm.set_layer_visible(id, visible);
                }
                let label = if locked {
                    format!("{name} (編集禁止)")
                } else {
                    format!("{name} (編集中)")
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
                "元に戻す {} / やり直し {}",
                self.vm.undo_len(),
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
            self.report(result, "画像を読み込みました");
        }
    }

    fn open_project(&mut self) {
        if let Some(path) = self.dialogs.open_project() {
            let result = self.vm.open_project(&path);
            self.report(result, "プロジェクトを開きました");
        }
    }

    fn save(&mut self) {
        match self.vm.save_project() {
            Ok(true) => self.message = "保存しました".to_owned(),
            Ok(false) => self.save_as(),
            Err(e) => self.message = e.to_string(),
        }
    }

    fn save_as(&mut self) {
        if let Some(path) = self.dialogs.save_project() {
            let result = self.vm.save_project_as(&path);
            self.report(result, "保存しました");
        }
    }

    fn export(&mut self, mode: ExportMode) {
        if let Some(path) = self.dialogs.export_png() {
            let result = self.vm.export_png(&path, mode);
            self.report(result, "PNG を書き出しました");
        }
    }

    fn delete_selection(&mut self) {
        let result = self.vm.delete_selection();
        self.report(result, "選択範囲を削除しました");
    }

    fn undo(&mut self) {
        let result = self.vm.undo();
        self.report(result.map(drop), "元に戻しました");
    }

    fn redo(&mut self) {
        let result = self.vm.redo();
        self.report(result.map(drop), "やり直しました");
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
        ToolKind::Pen => "ペン",
        ToolKind::Eraser => "消しゴム",
        ToolKind::Select => "範囲選択",
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
