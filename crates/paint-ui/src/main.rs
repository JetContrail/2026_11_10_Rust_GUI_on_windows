#![cfg_attr(not(test), windows_subsystem = "windows")]

use paint_ui::{APP_TITLE, NativeDialogs, PaintApp, default_view_model, install_system_fonts};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(APP_TITLE)
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };
    eframe::run_native(
        APP_TITLE,
        options,
        Box::new(|cc| {
            install_system_fonts(&cc.egui_ctx);
            Ok(Box::new(PaintApp::new(
                default_view_model(),
                Box::new(NativeDialogs),
            )))
        }),
    )
}
