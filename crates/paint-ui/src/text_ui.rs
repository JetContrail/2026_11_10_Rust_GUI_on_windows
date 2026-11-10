//! Text tool UI: on-canvas text input box, size input and font selection.

use eframe::egui::{self, Key, Modifiers, TextureHandle};
use paint_app::PaintViewModel;
use paint_domain::{CanvasSize, Color, DrawOp, Point, RasterImage, TextSize};

use crate::canvas::{CanvasView, to_color_image};

const PREVIEW_TEXT: &str = "Aa Bb あいう 漢字 123";
const PREVIEW_SIZE: CanvasSize = CanvasSize {
    width: 400,
    height: 64,
};

/// Text size (numeric input + presets) and font selection button.
/// Returns `true` when the font selection window should open.
pub(crate) fn text_settings(ui: &mut egui::Ui, vm: &mut PaintViewModel) -> bool {
    ui.horizontal(|ui| {
        let label = ui.label("文字サイズ");
        let mut px = vm.text_size().px();
        let response = ui
            .add(
                egui::DragValue::new(&mut px)
                    .range(TextSize::MIN..=TextSize::MAX)
                    .suffix(" px"),
            )
            .labelled_by(label.id);
        if response.changed() {
            vm.set_text_size(TextSize::clamped(px));
        }
    });
    ui.horizontal_wrapped(|ui| {
        for px in TextSize::PRESETS {
            if ui
                .selectable_label(vm.text_size().px() == px, px.to_string())
                .clicked()
            {
                vm.set_text_size(TextSize::clamped(px));
            }
        }
    });
    ui.label(format!("フォント: {}", vm.text_font()));
    ui.button("フォント選択…").clicked()
}

/// The on-canvas text input box of the text tool.
#[derive(Debug, Default)]
pub(crate) struct TextInput {
    /// Origin of the draft that already received keyboard focus.
    focused_origin: Option<Point>,
}

impl TextInput {
    /// Ctrl+Enter commits, Esc cancels, Enter inserts a newline. Japanese
    /// IME composition is handled by the focused `TextEdit`.
    pub(crate) fn show(
        &mut self,
        ctx: &egui::Context,
        vm: &mut PaintViewModel,
        canvas: &CanvasView,
    ) -> Result<(), String> {
        let Some(draft) = vm.text_draft().cloned() else {
            self.focused_origin = None;
            return Ok(());
        };
        let Some(anchor) = canvas.to_screen(draft.origin) else {
            return Ok(());
        };
        let id = egui::Id::unique("text-input");
        if ctx.memory(|m| m.has_focus(id)) {
            let (commit, cancel) = ctx.input_mut(|i| {
                (
                    i.consume_key(Modifiers::COMMAND, Key::Enter),
                    i.consume_key(Modifiers::NONE, Key::Escape),
                )
            });
            if commit {
                return vm.commit_text().map(|_| ()).map_err(|e| e.to_string());
            }
            if cancel {
                vm.cancel_text();
                return Ok(());
            }
        }
        let mut text = draft.text;
        egui::Area::new(egui::Id::unique("text-input-area"))
            .order(egui::Order::Foreground)
            .fixed_pos(anchor)
            .pivot(egui::Align2::LEFT_BOTTOM)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    let response = ui.add(
                        egui::TextEdit::multiline(&mut text)
                            .id(id)
                            .desired_rows(2)
                            .desired_width(260.0)
                            .hint_text("文字を入力 (Ctrl+Enter: 確定 / Esc: 取消)"),
                    );
                    if self.focused_origin != Some(draft.origin) {
                        response.request_focus();
                        // The click that opened the box would take the
                        // focus away again on release; retry until then.
                        let settled =
                            ui.input(|i| !i.pointer.any_down() && !i.pointer.any_released());
                        if settled {
                            self.focused_origin = Some(draft.origin);
                        }
                    }
                    if response.changed() {
                        vm.set_text_draft(&text);
                    }
                });
            });
        Ok(())
    }
}

/// The font selection window.
pub(crate) struct FontPicker {
    families: Vec<String>,
    filter: String,
    selected: String,
    preview: Option<(String, TextureHandle)>,
}

impl std::fmt::Debug for FontPicker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontPicker")
            .field("families", &self.families.len())
            .field("selected", &self.selected)
            .finish_non_exhaustive()
    }
}

impl FontPicker {
    pub(crate) fn new(vm: &PaintViewModel) -> Self {
        Self {
            families: vm.font_families(),
            filter: String::new(),
            selected: vm.text_font().to_owned(),
            preview: None,
        }
    }

    /// Returns `false` once the window is closed (OK or cancel).
    pub(crate) fn show(&mut self, ctx: &egui::Context, vm: &mut PaintViewModel) -> bool {
        let mut open = true;
        let mut close = false;
        egui::Window::new("フォントの選択")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let label = ui.label("検索");
                    ui.text_edit_singleline(&mut self.filter)
                        .labelled_by(label.id);
                });
                let needle = self.filter.to_lowercase();
                let shown: Vec<&String> = self
                    .families
                    .iter()
                    .filter(|f| f.to_lowercase().contains(&needle))
                    .collect();
                if self.families.is_empty() {
                    ui.label("インストール済みのフォントが見つかりません");
                }
                let row = ui.spacing().interact_size.y;
                egui::ScrollArea::vertical()
                    .max_height(260.0)
                    .auto_shrink([false, true])
                    .show_rows(ui, row, shown.len(), |ui, range| {
                        for name in &shown[range] {
                            if ui
                                .selectable_label(**name == self.selected, name.as_str())
                                .clicked()
                            {
                                self.selected.clone_from(name);
                            }
                        }
                    });
                ui.separator();
                ui.label(format!("プレビュー: {}", self.selected));
                let texture = self.preview_texture(ctx, vm);
                ui.image((texture.id(), texture.size_vec2()));
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        vm.set_text_font(&self.selected);
                        close = true;
                    }
                    if ui.button("キャンセル").clicked() {
                        close = true;
                    }
                });
            });
        open && !close
    }

    fn preview_texture(&mut self, ctx: &egui::Context, vm: &PaintViewModel) -> TextureHandle {
        if let Some((family, texture)) = &self.preview
            && *family == self.selected
        {
            return texture.clone();
        }
        let mut image = RasterImage::filled(PREVIEW_SIZE, Color::WHITE);
        let op = DrawOp::Text {
            text: PREVIEW_TEXT.to_owned(),
            origin: Point::new(6.0, 8.0),
            size: TextSize::clamped(36),
            color: Color::BLACK,
            font: self.selected.clone(),
        };
        paint_engine::render_op(&mut image, &op, vm.render_options());
        let texture = ctx.load_texture(
            "font-preview",
            to_color_image(&image),
            egui::TextureOptions::default(),
        );
        self.preview = Some((self.selected.clone(), texture.clone()));
        texture
    }
}
