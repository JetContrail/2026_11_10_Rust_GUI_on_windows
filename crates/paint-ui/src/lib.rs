//! UI layer (View) built with eframe/egui. `PaintApp` renders the state of
//! [`paint_app::PaintViewModel`] and forwards user input to it.

mod app;
mod canvas;
mod dialogs;
mod fonts;
pub mod i18n;
mod text_ui;

pub use app::{APP_TITLE, PaintApp};
pub use dialogs::{FileDialogs, NativeDialogs};
pub use fonts::install_system_fonts;

/// Builds the ViewModel with the file-system persistence adapters
/// (composition root).
pub fn default_view_model() -> paint_app::PaintViewModel {
    paint_app::PaintViewModel::new(paint_app::Services {
        project_store: Box::new(paint_persistence::JsonProjectStore),
        image_importer: Box::new(paint_persistence::FileImageImporter),
        image_exporter: Box::new(paint_persistence::PngExporter),
        fonts: std::sync::Arc::new(paint_persistence::SystemFontSource::new()),
    })
}
