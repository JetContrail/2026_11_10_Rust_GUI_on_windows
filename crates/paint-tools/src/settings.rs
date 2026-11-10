use paint_domain::{Color, StrokeWidth};

/// The tool selected in the tool palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToolKind {
    /// Freehand pen; a click without movement draws a dot.
    #[default]
    Pen,
    /// Freehand eraser.
    Eraser,
    /// Rectangle selection for range erase (Delete key).
    Select,
}

impl ToolKind {
    pub const ALL: [Self; 3] = [Self::Pen, Self::Eraser, Self::Select];
}

/// User-selected tool parameters (pen color / width, eraser size).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolSettings {
    pub color: Color,
    pub pen_width: StrokeWidth,
    pub eraser_width: StrokeWidth,
}

impl Default for ToolSettings {
    fn default() -> Self {
        Self {
            color: Color::BLACK,
            pen_width: StrokeWidth::Px4,
            eraser_width: StrokeWidth::Px16,
        }
    }
}
