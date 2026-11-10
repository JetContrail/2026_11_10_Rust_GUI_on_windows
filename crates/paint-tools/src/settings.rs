use paint_domain::{Color, StrokeWidth};

/// The tool selected in the tool palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToolKind {
    /// Freehand pen; a click without movement draws a dot.
    #[default]
    Pen,
    /// Straight line: press = start point, release = end point.
    Line,
    /// Freehand eraser.
    Eraser,
    /// Rectangle selection for range erase (Delete key).
    Select,
    /// Text: a click places a text box (edited by the ViewModel / UI).
    Text,
}

impl ToolKind {
    pub const ALL: [Self; 5] = [
        Self::Pen,
        Self::Line,
        Self::Eraser,
        Self::Select,
        Self::Text,
    ];
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
