//! Integration test of all non-GUI components wired together with the real
//! file-system adapters: ViewModel + Tools + Engine + Domain + Persistence.

#![allow(clippy::unwrap_used)] // test helpers outside #[test] fns
use std::path::Path;

use paint_app::{ExportMode, PaintViewModel, Services};
use paint_domain::ports::{ImageExporter, ImageImporter};
use paint_domain::{CanvasSize, Color, Document, Point, RasterImage, StrokeWidth};
use paint_persistence::{FileImageImporter, JsonProjectStore, PngExporter, encode_png};
use paint_tools::ToolKind;

const BLUE: Color = Color::rgb(0, 0, 255);

fn new_vm() -> PaintViewModel {
    PaintViewModel::new(Services {
        project_store: Box::new(JsonProjectStore),
        image_importer: Box::new(FileImageImporter),
        image_exporter: Box::new(PngExporter),
        fonts: std::sync::Arc::new(paint_domain::ports::NoFonts),
    })
}

fn drag(vm: &mut PaintViewModel, points: &[(f32, f32)]) {
    let p = |(x, y): (f32, f32)| Point::new(x, y);
    vm.pointer_pressed(p(points[0])).unwrap();
    for &pt in &points[1..] {
        vm.pointer_dragged(p(pt)).unwrap();
    }
    vm.pointer_released(p(*points.last().unwrap())).unwrap();
}

fn drawing_pixel(vm: &PaintViewModel, x: u32, y: u32) -> Color {
    vm.layer_cache(Document::DRAWING_LAYER)
        .unwrap()
        .raster()
        .pixel(x, y)
        .unwrap()
}

#[test]
fn draw_save_reload_undo_redo_and_export() {
    let dir = tempfile::tempdir().unwrap();

    // Underlay image (PNG on disk).
    let bg_path = dir.path().join("underlay.png");
    let size = CanvasSize::new(200, 100).unwrap();
    std::fs::write(
        &bg_path,
        encode_png(&RasterImage::filled(size, Color::WHITE)).unwrap(),
    )
    .unwrap();

    let mut vm = new_vm();
    assert_eq!(vm.canvas(), CanvasSize::DEFAULT);
    vm.import_image(&bg_path).unwrap();
    assert_eq!(vm.canvas(), size);

    // Freehand pen, dot, eraser, range erase.
    vm.set_color(BLUE);
    vm.set_pen_width(StrokeWidth::Px4);
    drag(&mut vm, &[(10.0, 50.0), (60.0, 50.0), (110.0, 50.0)]);
    drag(&mut vm, &[(150.0, 20.0)]);
    vm.select_tool(ToolKind::Eraser);
    vm.set_eraser_width(StrokeWidth::Px8);
    drag(&mut vm, &[(30.0, 40.0), (30.0, 60.0)]);
    vm.select_tool(ToolKind::Select);
    drag(&mut vm, &[(90.0, 0.0), (120.0, 100.0)]);
    vm.delete_selection().unwrap();
    assert_eq!(drawing_pixel(&vm, 60, 50), BLUE);
    assert_eq!(drawing_pixel(&vm, 30, 50), Color::TRANSPARENT);
    assert_eq!(drawing_pixel(&vm, 100, 50), Color::TRANSPARENT);
    assert_eq!(drawing_pixel(&vm, 150, 20), BLUE);

    // Save and reload in a fresh ViewModel.
    let project = dir.path().join("work.paint");
    vm.save_project_as(&project).unwrap();
    let mut reloaded = new_vm();
    reloaded.open_project(&project).unwrap();
    assert_eq!(reloaded.document(), vm.document());
    assert_eq!(
        reloaded
            .layer_cache(Document::DRAWING_LAYER)
            .unwrap()
            .raster(),
        vm.layer_cache(Document::DRAWING_LAYER).unwrap().raster()
    );

    // Undo everything (4 ops + image import), then redo.
    for _ in 0..5 {
        assert!(vm.undo().unwrap());
    }
    assert_eq!(vm.canvas(), CanvasSize::DEFAULT);
    assert_eq!(drawing_pixel(&vm, 60, 50), Color::TRANSPARENT);
    for _ in 0..5 {
        assert!(vm.redo().unwrap());
    }
    assert_eq!(vm.document(), reloaded.document());
    assert!(!vm.is_modified());

    // Transparent PNG export round trip.
    let png = dir.path().join("drawing.png");
    vm.export_png(&png, ExportMode::DrawingOnly).unwrap();
    let exported = FileImageImporter.import(&png).unwrap();
    assert_eq!(exported.size(), size);
    assert_eq!(exported.pixel(0, 0), Some(Color::TRANSPARENT));
    assert_eq!(exported.pixel(60, 50), Some(BLUE));

    let flat = dir.path().join("flat.png");
    vm.export_png(&flat, ExportMode::VisibleLayers).unwrap();
    assert_eq!(
        FileImageImporter.import(&flat).unwrap().pixel(0, 0),
        Some(Color::WHITE)
    );
}

#[test]
fn invalid_files_report_errors_without_changing_state() {
    let dir = tempfile::tempdir().unwrap();
    let mut vm = new_vm();
    drag(&mut vm, &[(10.0, 10.0)]);
    let before = vm.document().clone();

    let garbage = dir.path().join("garbage.paint");
    std::fs::write(&garbage, b"not json").unwrap();
    assert!(vm.open_project(&garbage).is_err());
    let not_image = dir.path().join("fake.png");
    std::fs::write(&not_image, b"not an image").unwrap();
    assert!(vm.import_image(&not_image).is_err());
    assert!(
        vm.save_project_as(&dir.path().join("no/dir/x.paint"))
            .is_err()
    );

    assert_eq!(vm.document(), &before);
    assert!(vm.undo().unwrap());
}

#[test]
fn exporter_writes_transparent_png_readable_by_importer() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.png");
    let mut img = RasterImage::transparent(CanvasSize::new(4, 4).unwrap());
    img.set_pixel(1, 1, Color::rgba(1, 2, 3, 4));
    PngExporter.export_png(&path, &img).unwrap();
    assert_eq!(FileImageImporter.import(&path).unwrap(), img);
    assert!(Path::new(&path).exists());
}

#[test]
fn text_is_saved_and_reloaded() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("text.paint");
    let mut vm = new_vm();
    vm.select_tool(ToolKind::Text);
    vm.set_color(BLUE);
    vm.set_text_size(paint_domain::TextSize::new(57).unwrap());
    vm.set_text_font("Meiryo");
    vm.pointer_pressed(Point::new(10.0, 20.0)).unwrap();
    vm.set_text_draft("こんにちは\nHello Ωμέγα Привет 你好 안녕");
    vm.commit_text().unwrap();
    vm.save_project_as(&path).unwrap();
    let before = vm
        .document()
        .ops(paint_domain::Document::DRAWING_LAYER)
        .unwrap()
        .to_vec();

    let mut reloaded = new_vm();
    reloaded.open_project(&path).unwrap();
    assert_eq!(
        reloaded
            .document()
            .ops(paint_domain::Document::DRAWING_LAYER)
            .unwrap(),
        before.as_slice()
    );
}
