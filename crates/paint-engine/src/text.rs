//! Text layout and rasterization with scalable (TrueType / OpenType) fonts.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};

use ab_glyph::{Font, FontVec, Glyph, PxScale, ScaleFont, point};
use paint_domain::ports::{FontSource, NoFonts};
use paint_domain::{CanvasSize, PixelRect, Point, TextSize};

/// Families tried, in order, for characters missing from the selected font
/// (Latin, Japanese, Simplified Chinese, Korean; Linux fallbacks last).
pub const FALLBACK_FAMILIES: [&str; 9] = [
    "Segoe UI",
    "Yu Gothic UI",
    "Meiryo",
    "Microsoft YaHei",
    "Malgun Gothic",
    "Segoe UI Symbol",
    "Noto Sans CJK JP",
    "Noto Sans",
    "DejaVu Sans",
];

/// Shared, lazily loaded font faces resolved by family name.
#[derive(Clone)]
pub struct FontLibrary {
    source: Arc<dyn FontSource>,
    loaded: Arc<Mutex<HashMap<String, Option<Arc<FontVec>>>>>,
}

impl fmt::Debug for FontLibrary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FontLibrary")
            .field("source", &self.source)
            .finish_non_exhaustive()
    }
}

impl Default for FontLibrary {
    fn default() -> Self {
        Self::new(Arc::new(NoFonts))
    }
}

impl PartialEq for FontLibrary {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.loaded, &other.loaded)
    }
}

impl FontLibrary {
    pub fn new(source: Arc<dyn FontSource>) -> Self {
        Self {
            source,
            loaded: Arc::default(),
        }
    }

    /// Installed font families (for the font selection UI).
    pub fn family_names(&self) -> Vec<String> {
        self.source.family_names()
    }

    fn face(&self, family: &str) -> Option<Arc<FontVec>> {
        let mut loaded = self.loaded.lock().unwrap_or_else(|e| e.into_inner());
        loaded
            .entry(family.to_owned())
            .or_insert_with(|| {
                let data = self.source.load(family)?;
                FontVec::try_from_vec_and_index(data.bytes.to_vec(), data.index)
                    .ok()
                    .map(Arc::new)
            })
            .clone()
    }

    /// `family` followed by the installed fallback families.
    fn chain(&self, family: &str) -> Vec<Arc<FontVec>> {
        std::iter::once(family)
            .chain(FALLBACK_FAMILIES.into_iter().filter(|f| *f != family))
            .filter_map(|f| self.face(f))
            .collect()
    }
}

struct Placed {
    font: Arc<FontVec>,
    glyph: Glyph,
}

/// Lays out `text` (lines split at `\n`) with its top-left at `origin`.
fn layout(
    fonts: &FontLibrary,
    family: &str,
    text: &str,
    origin: Point,
    size: TextSize,
) -> Vec<Placed> {
    let chain = fonts.chain(family);
    let Some(primary) = chain.first() else {
        return Vec::new();
    };
    let scale = PxScale::from(f32::from(size.px()));
    let metrics = primary.as_scaled(scale);
    let line_height = metrics.height() + metrics.line_gap();
    let mut placed = Vec::new();
    for (row, line) in text.split('\n').enumerate() {
        let baseline = origin.y + metrics.ascent() + row as f32 * line_height;
        let mut x = origin.x;
        let mut prev: Option<(usize, ab_glyph::GlyphId)> = None;
        for c in line.chars().filter(|c| !c.is_control()) {
            let (index, font) = chain
                .iter()
                .enumerate()
                .find(|(_, f)| f.glyph_id(c).0 != 0)
                .unwrap_or((0, primary));
            let scaled = font.as_scaled(scale);
            let id = font.glyph_id(c);
            if let Some((prev_index, prev_id)) = prev
                && prev_index == index
            {
                x += scaled.kern(prev_id, id);
            }
            placed.push(Placed {
                font: Arc::clone(font),
                glyph: id.with_scale_and_position(scale, point(x, baseline)),
            });
            x += scaled.h_advance(id);
            prev = Some((index, id));
        }
    }
    placed
}

/// Pixel area covered by the rendered text (clamped to the canvas).
pub(crate) fn text_bounds(
    fonts: &FontLibrary,
    family: &str,
    text: &str,
    origin: Point,
    size: TextSize,
    canvas: CanvasSize,
) -> PixelRect {
    let mut bounds: Option<ab_glyph::Rect> = None;
    for p in layout(fonts, family, text, origin, size) {
        if let Some(outline) = p.font.outline_glyph(p.glyph) {
            let r = outline.px_bounds();
            bounds = Some(bounds.map_or(r, |b| ab_glyph::Rect {
                min: point(b.min.x.min(r.min.x), b.min.y.min(r.min.y)),
                max: point(b.max.x.max(r.max.x), b.max.y.max(r.max.y)),
            }));
        }
    }
    bounds.map_or_else(PixelRect::default, |b| {
        PixelRect::from_corners(
            Point::new(b.min.x - 1.0, b.min.y - 1.0),
            Point::new(b.max.x + 1.0, b.max.y + 1.0),
            canvas,
        )
    })
}

/// Glyph coverage (0..=1) for every pixel of `bounds`.
pub(crate) fn text_mask(
    fonts: &FontLibrary,
    family: &str,
    text: &str,
    origin: Point,
    size: TextSize,
    bounds: PixelRect,
) -> Vec<f32> {
    let w = bounds.width as usize;
    let mut mask = vec![0.0f32; w * bounds.height as usize];
    for p in layout(fonts, family, text, origin, size) {
        let Some(outline) = p.font.outline_glyph(p.glyph) else {
            continue;
        };
        let r = outline.px_bounds();
        outline.draw(|gx, gy, cov| {
            let x = r.min.x as i64 + i64::from(gx) - i64::from(bounds.x);
            let y = r.min.y as i64 + i64::from(gy) - i64::from(bounds.y);
            if x < 0 || y < 0 || x >= w as i64 || y >= i64::from(bounds.height) {
                return;
            }
            let i = y as usize * w + x as usize;
            mask[i] = mask[i].max(cov.min(1.0));
        });
    }
    mask
}

#[cfg(test)]
mod tests {
    use paint_domain::ports::FontData;
    use paint_domain::{Color, DrawOp, RasterImage};

    use super::*;
    use crate::{RenderOptions, op_bounds, render_op};

    /// One Latin font ("Ubuntu") plus a symbol font used as fallback.
    #[derive(Debug)]
    struct TestFonts;

    impl FontSource for TestFonts {
        fn family_names(&self) -> Vec<String> {
            vec!["Hack".to_owned(), "Ubuntu".to_owned()]
        }
        fn load(&self, family: &str) -> Option<FontData> {
            let bytes: &[u8] = match family {
                "Ubuntu" => epaint_default_fonts::UBUNTU_LIGHT,
                // Stands in for a CJK fallback font.
                "Segoe UI Symbol" => epaint_default_fonts::NOTO_EMOJI_REGULAR,
                _ => return None,
            };
            Some(FontData {
                bytes: bytes.into(),
                index: 0,
            })
        }
    }

    fn options() -> RenderOptions {
        RenderOptions {
            fonts: FontLibrary::new(Arc::new(TestFonts)),
            ..RenderOptions::default()
        }
    }

    fn text(t: &str, size: u16) -> DrawOp {
        DrawOp::Text {
            text: t.to_owned(),
            origin: Point::new(10.0, 10.0),
            size: TextSize::clamped(size),
            color: Color::rgb(0, 0, 255),
            font: "Ubuntu".to_owned(),
        }
    }

    fn render(op: &DrawOp) -> RasterImage {
        let mut img = RasterImage::transparent(CanvasSize {
            width: 400,
            height: 300,
        });
        render_op(&mut img, op, &options());
        img
    }

    fn painted(img: &RasterImage) -> usize {
        img.pixels().chunks(4).filter(|p| p[3] > 0).count()
    }

    #[test]
    fn text_is_rendered_in_color_inside_bounds() {
        let op = text("Hello", 32);
        let img = render(&op);
        let bounds = op_bounds(&op, img.size(), &options());
        assert!(!bounds.is_empty());
        assert!(bounds.x >= 9 && bounds.y >= 9, "{bounds:?}");
        assert!(painted(&img) > 50);
        for y in 0..img.height() {
            for x in 0..img.width() {
                let c = img.pixel(x, y).unwrap();
                if c.a > 0 {
                    assert!(bounds.contains(x, y), "pixel {x},{y} outside {bounds:?}");
                    assert_eq!((c.r, c.g, c.b), (0, 0, 255));
                }
            }
        }
    }

    #[test]
    fn size_scales_the_text() {
        let small = op_bounds(&text("Hi", 16), CanvasSize::default(), &options());
        let large = op_bounds(&text("Hi", 64), CanvasSize::default(), &options());
        assert!(large.height > small.height * 3, "{small:?} {large:?}");
        assert!(large.width > small.width * 3);
    }

    #[test]
    fn newline_starts_a_new_line() {
        let one = op_bounds(&text("A", 32), CanvasSize::default(), &options());
        let two = op_bounds(&text("A\nA", 32), CanvasSize::default(), &options());
        assert!(two.height > one.height + 20, "{one:?} {two:?}");
    }

    #[test]
    fn missing_glyphs_use_fallback_font() {
        let lib = FontLibrary::new(Arc::new(TestFonts));
        let star = '\u{2B50}';
        let primary = lib.face("Ubuntu").unwrap();
        assert_eq!(primary.glyph_id(star).0, 0, "precondition");
        let placed = layout(
            &lib,
            "Ubuntu",
            &star.to_string(),
            Point::new(0.0, 0.0),
            TextSize::default(),
        );
        assert_ne!(placed[0].font.glyph_id(star).0, 0);
    }

    #[test]
    fn unknown_family_falls_back_and_no_fonts_draws_nothing() {
        let mut op = text("abc", 32);
        if let DrawOp::Text { font, .. } = &mut op {
            "Not Installed".clone_into(font);
        }
        assert!(painted(&render(&op)) > 0, "fallback family used");

        let mut img = RasterImage::transparent(CanvasSize::default());
        let rect = render_op(&mut img, &op, &RenderOptions::default());
        assert!(rect.is_empty());
        assert_eq!(painted(&img), 0);
    }
}
