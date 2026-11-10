//! Paint application entry point.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

const APP_TITLE: &str = "Rust Paint";

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(APP_TITLE)
            .with_inner_size([1280.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        APP_TITLE,
        options,
        Box::new(|_cc| Ok(Box::<PlaceholderApp>::default())),
    )
}

#[derive(Debug, Default)]
struct PlaceholderApp;

impl eframe::App for PlaceholderApp {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        ui.label(APP_TITLE);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn title_is_set() {
        assert!(!super::APP_TITLE.is_empty());
    }
}
