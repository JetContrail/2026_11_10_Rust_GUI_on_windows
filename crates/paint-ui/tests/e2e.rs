//! E2E test of the whole application through the egui UI (headless, via
//! `egui_kittest`): launch → draw → save → undo/redo → reload.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test helpers outside #[test] fns

use std::path::{Path, PathBuf};

use eframe::egui::{Event, Key, Modifiers, PointerButton, Pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use paint_app::ExportMode;
use paint_ui::{FileDialogs, PaintApp, default_view_model, install_system_fonts};

#[derive(Debug)]
struct FakeDialogs {
    project: PathBuf,
    png: PathBuf,
}

impl FileDialogs for FakeDialogs {
    fn open_image(&self) -> Option<PathBuf> {
        None
    }
    fn open_project(&self) -> Option<PathBuf> {
        Some(self.project.clone())
    }
    fn save_project(&self) -> Option<PathBuf> {
        Some(self.project.clone())
    }
    fn export_png(&self) -> Option<PathBuf> {
        Some(self.png.clone())
    }
}

fn launch(dir: &Path) -> Harness<'static, PaintApp> {
    let dialogs = FakeDialogs {
        project: dir.join("e2e.paint"),
        png: dir.join("e2e.png"),
    };
    let app = PaintApp::new(default_view_model(), Box::new(dialogs));
    let mut harness = Harness::builder()
        .with_size([1280.0, 800.0])
        .allow_missing_glyphs()
        .build_ui_state(|ui, app: &mut PaintApp| app.show(ui), app);
    let font = install_system_fonts(&harness.ctx);
    // Windows always ships a Japanese UI font; other CI hosts may draw tofu.
    if cfg!(windows) {
        assert!(font.is_some(), "Japanese system font not found");
    }
    harness.run();
    harness
}

fn screen(h: &Harness<'_, PaintApp>, x: f32, y: f32) -> Pos2 {
    h.state().canvas_to_screen(x, y).expect("canvas laid out")
}

fn pointer(h: &mut Harness<'_, PaintApp>, pos: Pos2, pressed: bool) {
    h.event(Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    });
    h.run();
}

fn stroke(h: &mut Harness<'_, PaintApp>, points: &[(f32, f32)]) {
    let pts: Vec<Pos2> = points.iter().map(|&(x, y)| screen(h, x, y)).collect();
    h.event(Event::PointerMoved(pts[0]));
    h.run();
    pointer(h, pts[0], true);
    for &p in &pts[1..] {
        h.event(Event::PointerMoved(p));
        h.run();
    }
    pointer(h, *pts.last().unwrap(), false);
}

fn ctrl(h: &mut Harness<'_, PaintApp>, key: Key) {
    h.key_press_modifiers(Modifiers::COMMAND, key);
    h.run();
}

#[test]
fn launch_draw_save_undo_redo_reload() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = launch(dir.path());
    // Launch: main window with all panels.
    for label in ["ツール", "レイヤー", "ファイル", "準備完了"] {
        h.get_by_label(label);
    }
    assert_eq!(h.state().view_model().undo_len(), 0);

    // Draw a freehand stroke and a dot.
    stroke(&mut h, &[(20.0, 20.0), (60.0, 40.0), (100.0, 20.0)]);
    stroke(&mut h, &[(200.0, 200.0)]);
    assert_eq!(h.state().view_model().undo_len(), 2);
    let drawn = h
        .state()
        .view_model()
        .render_export(ExportMode::DrawingOnly);
    assert!(drawn.pixel(200, 200).unwrap().a > 0, "dot drawn");

    // Save (Ctrl+S).
    ctrl(&mut h, Key::S);
    assert!(dir.path().join("e2e.paint").exists());
    assert!(!h.state().view_model().is_modified());

    // Undo / Redo (Ctrl+Z / Ctrl+Y).
    ctrl(&mut h, Key::Z);
    ctrl(&mut h, Key::Z);
    assert_eq!(h.state().view_model().undo_len(), 0);
    assert!(h.state().view_model().is_modified());
    ctrl(&mut h, Key::Y);
    ctrl(&mut h, Key::Y);
    assert_eq!(h.state().view_model().redo_len(), 0);
    assert_eq!(
        h.state()
            .view_model()
            .render_export(ExportMode::DrawingOnly),
        drawn
    );

    // Reload in a freshly launched app via File > Open project.
    let mut h2 = launch(dir.path());
    h2.get_by_label("ファイル").click();
    h2.run();
    h2.get_by_label("プロジェクトを開く…").click();
    h2.run();
    assert_eq!(h2.state().message(), "プロジェクトを開きました");
    assert_eq!(
        h2.state()
            .view_model()
            .render_export(ExportMode::DrawingOnly),
        drawn
    );
}

#[test]
fn shortcuts_select_delete_and_ctrl_wheel_zoom() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = launch(dir.path());
    stroke(&mut h, &[(10.0, 50.0), (150.0, 50.0)]);

    h.get_by_label("範囲選択").click();
    h.run();
    stroke(&mut h, &[(0.0, 0.0), (200.0, 100.0)]);
    h.key_press(Key::Delete);
    h.run();
    let img = h
        .state()
        .view_model()
        .render_export(ExportMode::DrawingOnly);
    assert_eq!(img.pixel(80, 50).unwrap().a, 0, "selection cleared");
    assert_eq!(h.state().view_model().undo_len(), 2);

    let over_canvas = screen(&h, 50.0, 50.0);
    h.event(Event::PointerMoved(over_canvas));
    h.event(Event::Zoom(2.0));
    h.run_steps(15); // scroll offset animates after zoom
    assert!(
        (h.state().zoom() - 2.0).abs() < 1e-3,
        "zoom {}",
        h.state().zoom()
    );
}

#[test]
fn transparent_png_export_from_menu() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = launch(dir.path());
    stroke(&mut h, &[(5.0, 5.0), (50.0, 5.0)]);
    h.get_by_label("ファイル").click();
    h.run();
    h.get_by_label("透過 PNG 書き出し (描画レイヤーのみ)…")
        .click();
    h.run();
    let bytes = std::fs::read(dir.path().join("e2e.png")).unwrap();
    let img = paint_persistence::decode_image(&bytes).unwrap();
    assert_eq!(img.pixel(500, 500).unwrap().a, 0, "background transparent");
    assert!(img.pixel(25, 5).unwrap().a > 0);
}

#[test]
fn line_tool_and_shift_straight_line() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = launch(dir.path());
    h.get_by_label("直線").click();
    h.run();
    stroke(&mut h, &[(10.0, 10.0), (50.0, 90.0), (100.0, 10.0)]);
    let img = h
        .state()
        .view_model()
        .render_export(ExportMode::DrawingOnly);
    assert!(img.pixel(55, 10).unwrap().a > 0, "start-to-end line");
    assert_eq!(img.pixel(50, 90).unwrap().a, 0, "drag path not drawn");

    h.get_by_label("ペン").click();
    h.run();
    h.event(Event::ModifiersChanged(Modifiers::SHIFT));
    stroke(&mut h, &[(10.0, 200.0), (50.0, 300.0), (100.0, 200.0)]);
    h.event(Event::ModifiersChanged(Modifiers::NONE));
    h.run();
    let img = h
        .state()
        .view_model()
        .render_export(ExportMode::DrawingOnly);
    assert!(img.pixel(55, 200).unwrap().a > 0, "Shift straight line");
    assert_eq!(img.pixel(50, 300).unwrap().a, 0, "detour not drawn");
    assert_eq!(h.state().view_model().undo_len(), 2);
}

fn open_project_from_menu(h: &mut Harness<'_, PaintApp>) {
    h.get_by_label("ファイル").click();
    h.run();
    h.get_by_label("プロジェクトを開く…").click();
    h.run();
}

#[test]
fn unsaved_changes_are_confirmed_before_opening_a_project() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = launch(dir.path());
    stroke(&mut h, &[(10.0, 10.0)]);
    ctrl(&mut h, Key::S);
    stroke(&mut h, &[(30.0, 30.0)]);
    assert!(h.state().view_model().is_modified());

    // Cancel keeps the unsaved drawing.
    open_project_from_menu(&mut h);
    assert!(h.state().is_confirming());
    h.get_by_label("未保存の変更があります");
    h.get_by_label("キャンセル").click();
    h.run();
    assert!(!h.state().is_confirming());
    assert_eq!(h.state().view_model().undo_len(), 2);

    // Discard opens the saved project (one dot).
    open_project_from_menu(&mut h);
    h.get_by_label("保存しない").click();
    h.run();
    assert!(!h.state().view_model().is_modified());
    assert_eq!(
        h.state()
            .view_model()
            .document()
            .ops(paint_domain::Document::DRAWING_LAYER)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn opening_without_changes_needs_no_confirmation() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = launch(dir.path());
    stroke(&mut h, &[(10.0, 10.0)]);
    ctrl(&mut h, Key::S);
    open_project_from_menu(&mut h);
    assert!(!h.state().is_confirming());
    assert_eq!(h.state().message(), "プロジェクトを開きました");
}

fn text_ops(h: &Harness<'_, PaintApp>) -> Vec<paint_domain::DrawOp> {
    h.state()
        .view_model()
        .document()
        .ops(paint_domain::Document::DRAWING_LAYER)
        .unwrap()
        .iter()
        .filter(|op| matches!(op, paint_domain::DrawOp::Text { .. }))
        .cloned()
        .collect()
}

fn type_text(h: &mut Harness<'_, PaintApp>, text: &str) {
    h.event(Event::Text(text.to_owned()));
    h.run();
}

#[test]
fn text_tool_enter_newline_ctrl_enter_commit_esc_cancel() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = launch(dir.path());
    h.get_by_label("テキスト").click();
    h.run();
    h.get_by_label("64").click();
    h.run();
    stroke(&mut h, &[(40.0, 120.0)]);
    assert!(h.state().view_model().text_draft().is_some());
    type_text(&mut h, "Hello");
    h.key_press(Key::Enter);
    h.run();
    type_text(&mut h, "日本語");
    assert_eq!(
        h.state().view_model().text_draft().unwrap().text,
        "Hello\n日本語"
    );
    h.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    h.run();
    assert!(h.state().view_model().text_draft().is_none());
    let ops = text_ops(&h);
    assert_eq!(ops.len(), 1);
    let paint_domain::DrawOp::Text { text, size, .. } = &ops[0] else {
        unreachable!()
    };
    assert_eq!((text.as_str(), size.px()), ("Hello\n日本語", 64));
    if cfg!(windows) {
        let img = h
            .state()
            .view_model()
            .render_export(ExportMode::DrawingOnly);
        assert!(img.pixels().chunks(4).any(|p| p[3] > 0), "text rendered");
    }

    stroke(&mut h, &[(300.0, 300.0)]);
    type_text(&mut h, "discard");
    h.key_press(Key::Escape);
    h.run();
    assert!(h.state().view_model().text_draft().is_none());
    assert_eq!(text_ops(&h).len(), 1);
    assert_eq!(h.state().view_model().undo_len(), 1);
}

#[test]
fn font_selection_window_changes_the_text_font() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = launch(dir.path());
    h.get_by_label("テキスト").click();
    h.run();
    h.get_by_label("フォント選択…").click();
    h.run();
    let families = h.state().view_model().font_families();
    let Some(name) = families.first().cloned() else {
        h.get_by_label("キャンセル").click();
        h.run();
        return;
    };
    h.get_all_by_label(&name).last().unwrap().click();
    h.run();
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(h.state().view_model().text_font(), name);
    assert!(h.query_by_label("OK").is_none(), "window closed");
}
