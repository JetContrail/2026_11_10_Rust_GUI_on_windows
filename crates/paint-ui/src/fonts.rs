use eframe::egui::{self, FontData, FontFamily};
use eframe::epaint::text::{FontInsert, FontPriority, InsertFontFamily};

/// Japanese-capable system fonts, tried in order. Loaded at runtime so the
/// executable stays small (fonts are not bundled).
const CANDIDATES: [&str; 5] = [
    r"C:\Windows\Fonts\YuGothM.ttc",
    r"C:\Windows\Fonts\meiryo.ttc",
    r"C:\Windows\Fonts\msgothic.ttc",
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/fonts-japanese-gothic.ttf",
];

/// Adds the first available Japanese font as a fallback after egui's
/// built-in fonts. Returns the path used, if any.
pub fn install_system_fonts(ctx: &egui::Context) -> Option<&'static str> {
    let (path, bytes) = CANDIDATES
        .iter()
        .find_map(|p| std::fs::read(p).ok().map(|b| (*p, b)))?;
    let families = [FontFamily::Proportional, FontFamily::Monospace]
        .into_iter()
        .map(|family| InsertFontFamily {
            family,
            priority: FontPriority::Lowest,
        })
        .collect();
    ctx.add_font(FontInsert::new(
        "system-ja",
        FontData::from_owned(bytes),
        families,
    ));
    Some(path)
}
