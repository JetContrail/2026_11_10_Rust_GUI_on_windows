use paint_domain::{CanvasSize, DrawOp, PixelRect, Point, RasterImage};

use crate::blend::{blend_over, erase};

/// Rendering quality options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderOptions {
    /// Smooth (anti-aliased) brush edges.
    pub antialias: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self { antialias: true }
    }
}

/// Pixel area an operation may modify (clamped to the canvas).
pub fn op_bounds(op: &DrawOp, canvas: CanvasSize) -> PixelRect {
    match op {
        DrawOp::Stroke { width, points, .. } | DrawOp::Erase { width, points } => {
            stroke_bounds(points, width.px() as f32 / 2.0, canvas)
        }
        DrawOp::ClearRect { rect } => rect.clamp_to(canvas),
    }
}

fn stroke_bounds(points: &[Point], radius: f32, canvas: CanvasSize) -> PixelRect {
    let finite = points.iter().filter(|p| p.is_finite());
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    let mut any = false;
    for p in finite {
        any = true;
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
    }
    if !any {
        return PixelRect::default();
    }
    let pad = radius + 1.0;
    PixelRect::from_corners(
        Point::new(min_x - pad, min_y - pad),
        Point::new(max_x + pad, max_y + pad),
        canvas,
    )
}

/// Renders `op` onto `target` and returns the modified area.
pub fn render_op(target: &mut RasterImage, op: &DrawOp, options: RenderOptions) -> PixelRect {
    let canvas = target.size();
    let bounds = op_bounds(op, canvas);
    if bounds.is_empty() {
        return bounds;
    }
    match op {
        DrawOp::Stroke {
            color,
            width,
            points,
        } => {
            let mask = coverage_mask(points, width.px() as f32 / 2.0, bounds, options);
            apply_mask(target, bounds, &mask, |px, c| {
                blend_over(px, color.to_array(), c);
            });
        }
        DrawOp::Erase { width, points } => {
            let mask = coverage_mask(points, width.px() as f32 / 2.0, bounds, options);
            apply_mask(target, bounds, &mask, erase);
        }
        DrawOp::ClearRect { rect } => {
            let stride = target.width() as usize * 4;
            let rect = rect.clamp_to(canvas);
            for y in rect.y..rect.bottom() {
                let start = y as usize * stride + rect.x as usize * 4;
                target.pixels_mut()[start..start + rect.width as usize * 4].fill(0);
            }
        }
    }
    bounds
}

/// Coverage (0..=1) of a round brush of `radius` swept along `points`,
/// for every pixel of `bounds`. Overlapping segments use the maximum so
/// joints are not blended twice.
fn coverage_mask(
    points: &[Point],
    radius: f32,
    bounds: PixelRect,
    options: RenderOptions,
) -> Vec<f32> {
    let w = bounds.width as usize;
    let mut mask = vec![0.0f32; w * bounds.height as usize];
    let points: Vec<Point> = points.iter().copied().filter(|p| p.is_finite()).collect();
    let segments: Vec<(Point, Point)> = match points.as_slice() {
        [] => return mask,
        [p] => vec![(*p, *p)],
        _ => points.windows(2).map(|s| (s[0], s[1])).collect(),
    };
    for (a, b) in segments {
        let seg = PixelRect::from_corners(
            Point::new(a.x.min(b.x) - radius - 1.0, a.y.min(b.y) - radius - 1.0),
            Point::new(a.x.max(b.x) + radius + 1.0, a.y.max(b.y) + radius + 1.0),
            CanvasSize {
                width: bounds.right(),
                height: bounds.bottom(),
            },
        );
        for y in seg.y.max(bounds.y)..seg.bottom() {
            for x in seg.x.max(bounds.x)..seg.right() {
                let c = Point::new(x as f32 + 0.5, y as f32 + 0.5);
                let d = distance_to_segment(c, a, b);
                let cov = if options.antialias {
                    (radius + 0.5 - d).clamp(0.0, 1.0)
                } else if d <= radius {
                    1.0
                } else {
                    0.0
                };
                let i = (y - bounds.y) as usize * w + (x - bounds.x) as usize;
                if cov > mask[i] {
                    mask[i] = cov;
                }
            }
        }
    }
    mask
}

fn apply_mask(
    target: &mut RasterImage,
    bounds: PixelRect,
    mask: &[f32],
    mut f: impl FnMut(&mut [u8], f32),
) {
    let stride = target.width() as usize * 4;
    let w = bounds.width as usize;
    let pixels = target.pixels_mut();
    for row in 0..bounds.height as usize {
        let y = bounds.y as usize + row;
        for col in 0..w {
            let cov = mask[row * w + col];
            if cov > 0.0 {
                let i = y * stride + (bounds.x as usize + col) * 4;
                f(&mut pixels[i..i + 4], cov);
            }
        }
    }
}

fn distance_to_segment(p: Point, a: Point, b: Point) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    if len2 <= f32::EPSILON {
        return p.distance(a);
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0);
    p.distance(Point::new(a.x + t * dx, a.y + t * dy))
}

#[cfg(test)]
mod tests {
    use paint_domain::{Color, StrokeWidth};

    use super::*;

    const RED: Color = Color::rgb(255, 0, 0);
    const AA: RenderOptions = RenderOptions { antialias: true };
    const CRISP: RenderOptions = RenderOptions { antialias: false };

    fn canvas() -> RasterImage {
        RasterImage::transparent(CanvasSize::new(100, 100).unwrap())
    }

    fn stroke(width: StrokeWidth, points: &[(f32, f32)]) -> DrawOp {
        DrawOp::Stroke {
            color: RED,
            width,
            points: points.iter().map(|&(x, y)| Point::new(x, y)).collect(),
        }
    }

    fn alpha(img: &RasterImage, x: u32, y: u32) -> u8 {
        img.pixel(x, y).unwrap().a
    }

    /// Number of opaque pixels in column `x`.
    fn opaque_rows(img: &RasterImage, x: u32) -> u32 {
        (0..img.height())
            .filter(|&y| alpha(img, x, y) == 255)
            .count() as u32
    }

    #[test]
    fn dot_is_drawn_at_click_position() {
        let mut img = canvas();
        render_op(&mut img, &stroke(StrokeWidth::Px1, &[(10.5, 20.5)]), CRISP);
        assert_eq!(img.pixel(10, 20), Some(RED));
        assert_eq!(alpha(&img, 11, 20), 0);
        assert_eq!(alpha(&img, 10, 21), 0);
    }

    #[test]
    fn horizontal_line_thickness_matches_every_width() {
        for width in StrokeWidth::ALL {
            // Odd widths are centered on a pixel center, even widths on a pixel edge.
            let y = if width.px() % 2 == 1 { 50.5 } else { 50.0 };
            let mut img = canvas();
            render_op(&mut img, &stroke(width, &[(5.0, y), (95.0, y)]), AA);
            assert_eq!(opaque_rows(&img, 50), width.px(), "width {width:?}");
        }
    }

    #[test]
    fn line_connects_start_and_end() {
        let mut img = canvas();
        render_op(
            &mut img,
            &stroke(StrokeWidth::Px2, &[(10.0, 10.0), (90.0, 90.0)]),
            AA,
        );
        assert_eq!(img.pixel(10, 10), Some(RED));
        assert_eq!(img.pixel(50, 50), Some(RED));
        assert_eq!(img.pixel(89, 89), Some(RED));
        assert_eq!(alpha(&img, 90, 10), 0);
    }

    #[test]
    fn antialiasing_produces_partial_alpha_on_edges() {
        let mut img = canvas();
        render_op(&mut img, &stroke(StrokeWidth::Px8, &[(50.3, 50.3)]), AA);
        let edge = alpha(&img, 54, 50);
        assert!(edge > 0 && edge < 255, "edge alpha {edge}");
        let mut crisp = canvas();
        render_op(
            &mut crisp,
            &stroke(StrokeWidth::Px8, &[(50.3, 50.3)]),
            CRISP,
        );
        assert!(crisp.pixels().chunks(4).all(|p| p[3] == 0 || p[3] == 255));
    }

    #[test]
    fn color_is_applied() {
        let mut img = canvas();
        let op = DrawOp::Stroke {
            color: Color::rgb(1, 2, 3),
            width: StrokeWidth::Px4,
            points: vec![Point::new(20.0, 20.0)],
        };
        render_op(&mut img, &op, AA);
        assert_eq!(img.pixel(20, 20), Some(Color::rgb(1, 2, 3)));
    }

    #[test]
    fn eraser_clears_pixels_along_path() {
        let mut img = RasterImage::filled(CanvasSize::new(100, 100).unwrap(), RED);
        let op = DrawOp::Erase {
            width: StrokeWidth::Px16,
            points: vec![Point::new(10.0, 50.0), Point::new(90.0, 50.0)],
        };
        render_op(&mut img, &op, AA);
        assert_eq!(img.pixel(50, 50), Some(Color::TRANSPARENT));
        assert_eq!(img.pixel(50, 43), Some(Color::TRANSPARENT));
        assert_eq!(img.pixel(50, 20), Some(RED));
        assert_eq!(img.pixel(1, 50), Some(RED));
    }

    #[test]
    fn clear_rect_clears_exact_area() {
        let mut img = RasterImage::filled(CanvasSize::new(10, 10).unwrap(), RED);
        let dirty = render_op(
            &mut img,
            &DrawOp::ClearRect {
                rect: PixelRect::new(2, 3, 4, 5),
            },
            AA,
        );
        assert_eq!(dirty, PixelRect::new(2, 3, 4, 5));
        assert_eq!(img.pixel(2, 3), Some(Color::TRANSPARENT));
        assert_eq!(img.pixel(5, 7), Some(Color::TRANSPARENT));
        assert_eq!(img.pixel(6, 7), Some(RED));
        assert_eq!(img.pixel(5, 8), Some(RED));
    }

    #[test]
    fn out_of_canvas_input_is_clipped_safely() {
        let mut img = canvas();
        let dirty = render_op(
            &mut img,
            &stroke(StrokeWidth::Px32, &[(-500.0, -500.0), (1e6, 50.0)]),
            AA,
        );
        assert!(dirty.right() <= 100 && dirty.bottom() <= 100);
        let nothing = render_op(&mut img, &stroke(StrokeWidth::Px4, &[(f32::NAN, 1.0)]), AA);
        assert!(nothing.is_empty());
        let outside = render_op(&mut img, &stroke(StrokeWidth::Px4, &[(500.0, 500.0)]), AA);
        assert!(outside.is_empty());
    }

    #[test]
    fn bounds_cover_brush_radius() {
        let size = CanvasSize::new(100, 100).unwrap();
        let b = op_bounds(&stroke(StrokeWidth::Px16, &[(50.0, 50.0)]), size);
        assert!(b.contains(42, 42) && b.contains(57, 57));
    }
}
