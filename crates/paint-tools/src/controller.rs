use paint_domain::{CanvasSize, DrawOp, PixelRect, Point};

use crate::{ToolKind, ToolSettings};

/// Minimum distance (canvas px) between recorded stroke points. Filters
/// redundant pointer samples without visibly changing the stroke.
const MIN_POINT_DISTANCE: f32 = 0.5;

/// What the application layer must do in response to an input event.
#[derive(Debug, Clone, PartialEq)]
pub enum ToolEvent {
    /// Show `op` as an uncommitted preview (replaces any previous preview).
    Preview(DrawOp),
    /// Commit `op` to the drawing layer (undoable); removes the preview.
    Commit(DrawOp),
    /// Discard the preview without committing.
    CancelPreview,
    /// The selection rectangle changed (`None` = no selection).
    Selection(Option<PixelRect>),
}

#[derive(Debug, Clone, PartialEq)]
enum Gesture {
    Idle,
    Stroke { points: Vec<Point> },
    Selecting { anchor: Point },
}

/// State machine turning pointer input into [`ToolEvent`]s.
///
/// All points are in canvas pixel coordinates (the UI converts screen
/// coordinates using the zoom/scroll transform before calling).
#[derive(Debug, Clone)]
pub struct ToolController {
    tool: ToolKind,
    settings: ToolSettings,
    gesture: Gesture,
    selection: Option<PixelRect>,
}

impl Default for ToolController {
    fn default() -> Self {
        Self::new(ToolSettings::default())
    }
}

impl ToolController {
    pub fn new(settings: ToolSettings) -> Self {
        Self {
            tool: ToolKind::default(),
            settings,
            gesture: Gesture::Idle,
            selection: None,
        }
    }

    pub const fn tool(&self) -> ToolKind {
        self.tool
    }

    pub const fn settings(&self) -> ToolSettings {
        self.settings
    }

    pub fn settings_mut(&mut self) -> &mut ToolSettings {
        &mut self.settings
    }

    pub const fn selection(&self) -> Option<PixelRect> {
        self.selection
    }

    /// `true` while a drag gesture is in progress.
    pub fn is_active(&self) -> bool {
        self.gesture != Gesture::Idle
    }

    /// Switches tool, cancelling any gesture in progress. The selection is
    /// kept so the user can still press Delete after switching.
    pub fn set_tool(&mut self, tool: ToolKind) -> Vec<ToolEvent> {
        let events = self.cancel();
        self.tool = tool;
        events
    }

    /// Pointer button pressed at `p`.
    pub fn press(&mut self, p: Point, _canvas: CanvasSize) -> Vec<ToolEvent> {
        let mut events = self.cancel();
        if !p.is_finite() {
            return events;
        }
        match self.tool {
            ToolKind::Pen | ToolKind::Eraser => {
                self.gesture = Gesture::Stroke { points: vec![p] };
                events.extend(self.stroke_op().map(ToolEvent::Preview));
            }
            ToolKind::Select => {
                self.gesture = Gesture::Selecting { anchor: p };
                events.extend(self.set_selection(None));
            }
        }
        events
    }

    /// Pointer moved to `p` while the button is held.
    pub fn drag(&mut self, p: Point, canvas: CanvasSize) -> Vec<ToolEvent> {
        if !p.is_finite() {
            return Vec::new();
        }
        match &mut self.gesture {
            Gesture::Idle => Vec::new(),
            Gesture::Stroke { points } => {
                if points
                    .last()
                    .is_some_and(|last| last.distance(p) < MIN_POINT_DISTANCE)
                {
                    return Vec::new();
                }
                points.push(p);
                self.stroke_op()
                    .map(ToolEvent::Preview)
                    .into_iter()
                    .collect()
            }
            Gesture::Selecting { anchor } => {
                let rect = PixelRect::from_corners(*anchor, p, canvas);
                let rect = (!rect.is_empty()).then_some(rect);
                self.set_selection(rect).into_iter().collect()
            }
        }
    }

    /// Pointer button released at `p`. A press/release without movement
    /// commits a single-point stroke (a dot) for pen and eraser.
    pub fn release(&mut self, p: Point, canvas: CanvasSize) -> Vec<ToolEvent> {
        let mut events = self.drag(p, canvas);
        let gesture = std::mem::replace(&mut self.gesture, Gesture::Idle);
        if let Gesture::Stroke { points } = gesture {
            events.extend(self.make_stroke(points).map(ToolEvent::Commit));
        }
        events
    }

    /// Aborts the gesture in progress (e.g. pointer capture lost).
    pub fn cancel(&mut self) -> Vec<ToolEvent> {
        match std::mem::replace(&mut self.gesture, Gesture::Idle) {
            Gesture::Stroke { .. } => vec![ToolEvent::CancelPreview],
            Gesture::Idle | Gesture::Selecting { .. } => Vec::new(),
        }
    }

    /// Delete key: range-erase the selection. The selection is kept so
    /// the result is visible in place; returns nothing without selection.
    pub fn delete_selection(&self) -> Option<ToolEvent> {
        self.selection
            .filter(|rect| !rect.is_empty())
            .map(|rect| ToolEvent::Commit(DrawOp::ClearRect { rect }))
    }

    /// Clears the selection (Esc).
    pub fn clear_selection(&mut self) -> Option<ToolEvent> {
        self.set_selection(None)
    }

    fn set_selection(&mut self, rect: Option<PixelRect>) -> Option<ToolEvent> {
        (self.selection != rect).then(|| {
            self.selection = rect;
            ToolEvent::Selection(rect)
        })
    }

    fn stroke_op(&self) -> Option<DrawOp> {
        match &self.gesture {
            Gesture::Stroke { points } => self.make_stroke(points.clone()),
            Gesture::Idle | Gesture::Selecting { .. } => None,
        }
    }

    fn make_stroke(&self, points: Vec<Point>) -> Option<DrawOp> {
        if points.is_empty() {
            return None;
        }
        Some(match self.tool {
            ToolKind::Pen => DrawOp::Stroke {
                color: self.settings.color,
                width: self.settings.pen_width,
                points,
            },
            ToolKind::Eraser => DrawOp::Erase {
                width: self.settings.eraser_width,
                points,
            },
            ToolKind::Select => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use paint_domain::{Color, StrokeWidth};

    use super::*;

    const CANVAS: CanvasSize = CanvasSize {
        width: 100,
        height: 80,
    };

    fn pt(x: f32, y: f32) -> Point {
        Point::new(x, y)
    }

    fn committed(events: &[ToolEvent]) -> Vec<&DrawOp> {
        events
            .iter()
            .filter_map(|e| match e {
                ToolEvent::Commit(op) => Some(op),
                _ => None,
            })
            .collect()
    }

    fn pen() -> ToolController {
        ToolController::new(ToolSettings {
            color: Color::rgb(255, 0, 0),
            pen_width: StrokeWidth::Px8,
            eraser_width: StrokeWidth::Px32,
        })
    }

    #[test]
    fn click_draws_a_dot() {
        let mut t = pen();
        let press = t.press(pt(10.0, 10.0), CANVAS);
        assert!(matches!(press.as_slice(), [ToolEvent::Preview(_)]));
        let events = t.release(pt(10.0, 10.0), CANVAS);
        assert_eq!(
            committed(&events),
            [&DrawOp::Stroke {
                color: Color::rgb(255, 0, 0),
                width: StrokeWidth::Px8,
                points: vec![pt(10.0, 10.0)],
            }]
        );
        assert!(!t.is_active());
    }

    #[test]
    fn drag_records_freehand_points_with_preview() {
        let mut t = pen();
        t.press(pt(0.0, 0.0), CANVAS);
        assert!(matches!(
            t.drag(pt(5.0, 5.0), CANVAS).as_slice(),
            [ToolEvent::Preview(op)] if op.points().len() == 2
        ));
        assert!(
            t.drag(pt(5.1, 5.1), CANVAS).is_empty(),
            "sub-pixel jitter is ignored"
        );
        let events = t.release(pt(9.0, 3.0), CANVAS);
        assert_eq!(
            committed(&events)[0].points(),
            [pt(0.0, 0.0), pt(5.0, 5.0), pt(9.0, 3.0)]
        );
    }

    #[test]
    fn eraser_uses_eraser_width() {
        let mut t = pen();
        t.set_tool(ToolKind::Eraser);
        t.press(pt(1.0, 1.0), CANVAS);
        t.drag(pt(20.0, 1.0), CANVAS);
        let events = t.release(pt(20.0, 1.0), CANVAS);
        assert_eq!(
            committed(&events),
            [&DrawOp::Erase {
                width: StrokeWidth::Px32,
                points: vec![pt(1.0, 1.0), pt(20.0, 1.0)],
            }]
        );
    }

    #[test]
    fn settings_changes_apply_to_next_stroke() {
        let mut t = pen();
        t.settings_mut().color = Color::rgb(0, 0, 255);
        t.settings_mut().pen_width = StrokeWidth::Px1;
        t.press(pt(1.0, 1.0), CANVAS);
        let events = t.release(pt(1.0, 1.0), CANVAS);
        assert!(matches!(
            committed(&events)[0],
            DrawOp::Stroke { color, width: StrokeWidth::Px1, .. } if *color == Color::rgb(0, 0, 255)
        ));
    }

    #[test]
    fn cancel_and_tool_switch_discard_the_stroke() {
        let mut t = pen();
        t.press(pt(1.0, 1.0), CANVAS);
        assert_eq!(t.set_tool(ToolKind::Eraser), [ToolEvent::CancelPreview]);
        assert!(t.release(pt(2.0, 2.0), CANVAS).is_empty());
        t.press(pt(1.0, 1.0), CANVAS);
        assert_eq!(t.cancel(), [ToolEvent::CancelPreview]);
        assert!(t.cancel().is_empty());
    }

    #[test]
    fn moves_without_press_are_ignored() {
        let mut t = pen();
        assert!(t.drag(pt(3.0, 3.0), CANVAS).is_empty());
        assert!(t.release(pt(3.0, 3.0), CANVAS).is_empty());
    }

    #[test]
    fn non_finite_points_are_ignored() {
        let mut t = pen();
        assert!(t.press(pt(f32::NAN, 0.0), CANVAS).is_empty());
        t.press(pt(1.0, 1.0), CANVAS);
        assert!(t.drag(pt(f32::INFINITY, 0.0), CANVAS).is_empty());
        let events = t.release(pt(1.0, f32::NAN), CANVAS);
        assert_eq!(committed(&events)[0].points(), [pt(1.0, 1.0)]);
    }

    #[test]
    fn selection_drag_then_delete_clears_rect() {
        let mut t = pen();
        t.set_tool(ToolKind::Select);
        assert!(t.press(pt(30.0, 20.0), CANVAS).is_empty());
        let rect = PixelRect::new(10, 5, 20, 15);
        assert_eq!(
            t.drag(pt(10.2, 5.7), CANVAS),
            [ToolEvent::Selection(Some(rect))]
        );
        assert!(t.release(pt(10.2, 5.7), CANVAS).is_empty());
        assert_eq!(t.selection(), Some(rect));
        assert_eq!(
            t.delete_selection(),
            Some(ToolEvent::Commit(DrawOp::ClearRect { rect }))
        );
        assert_eq!(t.selection(), Some(rect), "selection stays after delete");
    }

    #[test]
    fn selection_is_clamped_to_canvas() {
        let mut t = pen();
        t.set_tool(ToolKind::Select);
        t.press(pt(-50.0, -50.0), CANVAS);
        t.release(pt(500.0, 500.0), CANVAS);
        assert_eq!(t.selection(), Some(PixelRect::new(0, 0, 100, 80)));
    }

    #[test]
    fn click_without_drag_clears_selection() {
        let mut t = pen();
        t.set_tool(ToolKind::Select);
        t.press(pt(1.0, 1.0), CANVAS);
        t.release(pt(10.0, 10.0), CANVAS);
        assert!(t.selection().is_some());
        assert_eq!(
            t.press(pt(50.0, 50.0), CANVAS),
            [ToolEvent::Selection(None)]
        );
        t.release(pt(50.0, 50.0), CANVAS);
        assert_eq!(t.selection(), None);
        assert_eq!(t.delete_selection(), None);
    }

    #[test]
    fn esc_clears_selection() {
        let mut t = pen();
        t.set_tool(ToolKind::Select);
        t.press(pt(1.0, 1.0), CANVAS);
        t.release(pt(10.0, 10.0), CANVAS);
        assert!(matches!(
            t.clear_selection(),
            Some(ToolEvent::Selection(None))
        ));
        assert_eq!(t.clear_selection(), None);
    }

    #[test]
    fn selection_survives_tool_switch() {
        let mut t = pen();
        t.set_tool(ToolKind::Select);
        t.press(pt(1.0, 1.0), CANVAS);
        t.release(pt(10.0, 10.0), CANVAS);
        t.set_tool(ToolKind::Pen);
        assert!(t.delete_selection().is_some());
    }

    #[test]
    fn long_stroke_keeps_all_points() {
        let mut t = pen();
        t.press(pt(0.0, 0.0), CANVAS);
        for i in 1..=1000 {
            t.drag(pt(i as f32 * 0.1, (i % 7) as f32), CANVAS);
        }
        let events = t.release(pt(99.0, 0.0), CANVAS);
        assert!(committed(&events)[0].points().len() > 500);
    }
}
