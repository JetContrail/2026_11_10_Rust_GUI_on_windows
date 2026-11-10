use crate::CanvasSize;

/// A position in canvas coordinates (1.0 = one canvas pixel).
///
/// Pixel `(x, y)` covers the area `[x, x + 1) × [y, y + 1)`; its center is
/// `(x + 0.5, y + 0.5)`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn distance(self, other: Self) -> f32 {
        (self.x - other.x).hypot(self.y - other.y)
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

/// An axis-aligned rectangle of whole pixels (`width`/`height` may be 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl PixelRect {
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// The rectangle spanned by two corner points (any order), snapped
    /// outward to whole pixels and clamped to the canvas.
    pub fn from_corners(a: Point, b: Point, canvas: CanvasSize) -> Self {
        let clamp = |v: f32, max: u32| -> u32 {
            if v.is_nan() {
                0
            } else {
                v.clamp(0.0, max as f32) as u32
            }
        };
        let x0 = clamp(a.x.min(b.x).floor(), canvas.width);
        let y0 = clamp(a.y.min(b.y).floor(), canvas.height);
        let x1 = clamp(a.x.max(b.x).ceil(), canvas.width);
        let y1 = clamp(a.y.max(b.y).ceil(), canvas.height);
        Self::new(x0, y0, x1 - x0, y1 - y0)
    }

    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }

    pub const fn right(self) -> u32 {
        self.x + self.width
    }

    pub const fn bottom(self) -> u32 {
        self.y + self.height
    }

    pub fn contains(self, x: u32, y: u32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    /// Smallest rectangle containing both (empty rectangles are ignored).
    pub fn union(self, other: Self) -> Self {
        if self.is_empty() {
            return other;
        }
        if other.is_empty() {
            return self;
        }
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Self::new(
            x,
            y,
            self.right().max(other.right()) - x,
            self.bottom().max(other.bottom()) - y,
        )
    }

    /// Clamps the rectangle to the canvas bounds.
    pub fn clamp_to(self, canvas: CanvasSize) -> Self {
        let x = self.x.min(canvas.width);
        let y = self.y.min(canvas.height);
        let right = self.right().min(canvas.width);
        let bottom = self.bottom().min(canvas.height);
        Self::new(x, y, right.saturating_sub(x), bottom.saturating_sub(y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CANVAS: CanvasSize = CanvasSize {
        width: 100,
        height: 50,
    };

    #[test]
    fn from_corners_normalizes_order() {
        let r = PixelRect::from_corners(Point::new(30.2, 40.0), Point::new(10.7, 5.5), CANVAS);
        assert_eq!(r, PixelRect::new(10, 5, 21, 35));
    }

    #[test]
    fn from_corners_clamps_to_canvas() {
        let r = PixelRect::from_corners(Point::new(-10.0, -3.0), Point::new(500.0, 70.0), CANVAS);
        assert_eq!(r, PixelRect::new(0, 0, 100, 50));
        let nan = PixelRect::from_corners(Point::new(f32::NAN, 0.0), Point::new(1.0, 1.0), CANVAS);
        assert!(nan.right() <= CANVAS.width && nan.bottom() <= CANVAS.height);
    }

    #[test]
    fn union_and_contains() {
        let a = PixelRect::new(0, 0, 10, 10);
        let b = PixelRect::new(5, 5, 10, 10);
        let u = a.union(b);
        assert_eq!(u, PixelRect::new(0, 0, 15, 15));
        assert!(u.contains(14, 14));
        assert!(!u.contains(15, 0));
        assert_eq!(a.union(PixelRect::default()), a);
        assert_eq!(PixelRect::default().union(b), b);
    }

    #[test]
    fn clamp_to_canvas() {
        let r = PixelRect::new(90, 40, 20, 20).clamp_to(CANVAS);
        assert_eq!(r, PixelRect::new(90, 40, 10, 10));
        assert!(PixelRect::new(200, 0, 5, 5).clamp_to(CANVAS).is_empty());
    }

    #[test]
    fn point_distance() {
        assert_eq!(Point::new(0.0, 0.0).distance(Point::new(3.0, 4.0)), 5.0);
        assert!(!Point::new(f32::INFINITY, 0.0).is_finite());
    }
}
