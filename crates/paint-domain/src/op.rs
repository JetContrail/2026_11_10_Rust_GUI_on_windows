use crate::{Color, PixelRect, Point, StrokeWidth};

/// A single drawing operation applied to a vector (drawing) layer.
///
/// The drawing layer content is the ordered list of these operations; the
/// drawing engine rasterizes them in order.
#[derive(Debug, Clone, PartialEq)]
pub enum DrawOp {
    /// Pen stroke, dot (one point) or straight line (two points).
    Stroke {
        color: Color,
        width: StrokeWidth,
        points: Vec<Point>,
    },
    /// Eraser stroke: makes covered pixels transparent.
    Erase {
        width: StrokeWidth,
        points: Vec<Point>,
    },
    /// Range erase: makes the rectangle transparent.
    ClearRect { rect: PixelRect },
}

impl DrawOp {
    /// Points of the operation (empty for [`DrawOp::ClearRect`]).
    pub fn points(&self) -> &[Point] {
        match self {
            Self::Stroke { points, .. } | Self::Erase { points, .. } => points,
            Self::ClearRect { .. } => &[],
        }
    }

    /// `true` when all coordinates are finite and strokes have a point.
    pub fn is_valid(&self) -> bool {
        match self {
            Self::Stroke { points, .. } | Self::Erase { points, .. } => {
                !points.is_empty() && points.iter().all(|p| p.is_finite())
            }
            Self::ClearRect { .. } => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validity() {
        let stroke = |points| DrawOp::Stroke {
            color: Color::BLACK,
            width: StrokeWidth::Px1,
            points,
        };
        assert!(stroke(vec![Point::new(1.0, 1.0)]).is_valid());
        assert!(!stroke(vec![]).is_valid());
        assert!(!stroke(vec![Point::new(f32::NAN, 1.0)]).is_valid());
        assert!(
            DrawOp::ClearRect {
                rect: PixelRect::default()
            }
            .is_valid()
        );
    }

    #[test]
    fn points_accessor() {
        let erase = DrawOp::Erase {
            width: StrokeWidth::Px8,
            points: vec![Point::new(0.0, 0.0), Point::new(1.0, 1.0)],
        };
        assert_eq!(erase.points().len(), 2);
        let clear = DrawOp::ClearRect {
            rect: PixelRect::new(0, 0, 1, 1),
        };
        assert!(clear.points().is_empty());
    }
}
