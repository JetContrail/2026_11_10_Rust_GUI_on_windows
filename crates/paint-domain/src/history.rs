use std::collections::VecDeque;

use crate::{Document, DomainError, DrawOp, LayerId, Underlay};

/// Default number of undo steps kept (requirement: at least 50).
pub const DEFAULT_HISTORY_LIMIT: usize = 100;

/// An undoable edit of a [`Document`].
#[derive(Debug, Clone, PartialEq)]
pub enum EditCommand {
    AddOp { layer: LayerId, op: DrawOp },
    ReplaceUnderlay { before: Underlay, after: Underlay },
}

impl EditCommand {
    fn apply(&self, doc: &mut Document) -> Result<(), DomainError> {
        match self {
            Self::AddOp { layer, op } => doc.push_op(*layer, op.clone()),
            Self::ReplaceUnderlay { after, .. } => doc.replace_underlay(after.clone()).map(drop),
        }
    }

    fn revert(&self, doc: &mut Document) -> Result<(), DomainError> {
        match self {
            Self::AddOp { layer, .. } => doc.pop_op(*layer).map(drop),
            Self::ReplaceUnderlay { before, .. } => doc.replace_underlay(before.clone()).map(drop),
        }
    }
}

/// Linear undo/redo history with a bounded number of undo steps.
#[derive(Debug, Clone)]
pub struct History {
    undo: VecDeque<EditCommand>,
    redo: Vec<EditCommand>,
    limit: usize,
}

impl History {
    /// Creates a history keeping at most `limit` undo steps (minimum 1).
    pub fn new(limit: usize) -> Self {
        Self {
            undo: VecDeque::new(),
            redo: Vec::new(),
            limit: limit.max(1),
        }
    }

    pub const fn limit(&self) -> usize {
        self.limit
    }

    /// Applies `command` to `doc` and records it. Clears the redo stack.
    /// Nothing is recorded when the command fails.
    pub fn execute(&mut self, doc: &mut Document, command: EditCommand) -> Result<(), DomainError> {
        command.apply(doc)?;
        self.undo.push_back(command);
        while self.undo.len() > self.limit {
            self.undo.pop_front();
        }
        self.redo.clear();
        Ok(())
    }

    /// Reverts the last command. Returns the reverted command, `None` when
    /// there is nothing to undo.
    pub fn undo(&mut self, doc: &mut Document) -> Result<Option<&EditCommand>, DomainError> {
        let Some(command) = self.undo.pop_back() else {
            return Ok(None);
        };
        if let Err(e) = command.revert(doc) {
            self.undo.push_back(command);
            return Err(e);
        }
        self.redo.push(command);
        Ok(self.redo.last())
    }

    /// Re-applies the last undone command.
    pub fn redo(&mut self, doc: &mut Document) -> Result<Option<&EditCommand>, DomainError> {
        let Some(command) = self.redo.pop() else {
            return Ok(None);
        };
        if let Err(e) = command.apply(doc) {
            self.redo.push(command);
            return Err(e);
        }
        self.undo.push_back(command);
        Ok(self.undo.back())
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

impl Default for History {
    fn default() -> Self {
        Self::new(DEFAULT_HISTORY_LIMIT)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{CanvasSize, Color, Point, RasterImage, StrokeWidth};

    fn add(i: usize) -> EditCommand {
        EditCommand::AddOp {
            layer: Document::DRAWING_LAYER,
            op: DrawOp::Stroke {
                color: Color::BLACK,
                width: StrokeWidth::Px2,
                points: vec![Point::new(i as f32, 0.0)],
            },
        }
    }

    fn op_count(doc: &Document) -> usize {
        doc.ops(Document::DRAWING_LAYER).unwrap().len()
    }

    #[test]
    fn undo_redo_single_step() {
        let mut doc = Document::default();
        let mut h = History::default();
        h.execute(&mut doc, add(0)).unwrap();
        assert_eq!(op_count(&doc), 1);
        assert_eq!(h.undo(&mut doc).unwrap(), Some(&add(0)));
        assert_eq!(op_count(&doc), 0);
        assert_eq!(h.redo(&mut doc).unwrap(), Some(&add(0)));
        assert_eq!(op_count(&doc), 1);
    }

    #[test]
    fn undo_and_redo_at_least_50_times() {
        let mut doc = Document::default();
        let mut h = History::default();
        for i in 0..60 {
            h.execute(&mut doc, add(i)).unwrap();
        }
        for _ in 0..60 {
            assert!(h.undo(&mut doc).unwrap().is_some());
        }
        assert_eq!(op_count(&doc), 0);
        for _ in 0..60 {
            assert!(h.redo(&mut doc).unwrap().is_some());
        }
        assert_eq!(op_count(&doc), 60);
        assert_eq!(doc.ops(Document::DRAWING_LAYER).unwrap()[59], {
            let EditCommand::AddOp { op, .. } = add(59) else {
                unreachable!()
            };
            op
        });
    }

    #[test]
    fn limit_discards_oldest_steps() {
        let mut doc = Document::default();
        let mut h = History::new(50);
        for i in 0..70 {
            h.execute(&mut doc, add(i)).unwrap();
        }
        assert_eq!(h.undo_len(), 50);
        while h.undo(&mut doc).unwrap().is_some() {}
        // The 20 oldest operations are kept in the document permanently.
        assert_eq!(op_count(&doc), 20);
    }

    #[test]
    fn new_command_clears_redo() {
        let mut doc = Document::default();
        let mut h = History::default();
        h.execute(&mut doc, add(0)).unwrap();
        h.undo(&mut doc).unwrap();
        assert!(h.can_redo());
        h.execute(&mut doc, add(1)).unwrap();
        assert!(!h.can_redo());
        assert_eq!(h.redo(&mut doc).unwrap(), None);
    }

    #[test]
    fn empty_history_is_noop() {
        let mut doc = Document::default();
        let mut h = History::default();
        assert!(!h.can_undo());
        assert_eq!(h.undo(&mut doc).unwrap(), None);
        assert_eq!(h.redo(&mut doc).unwrap(), None);
    }

    #[test]
    fn failed_command_is_not_recorded() {
        let mut doc = Document::default();
        let mut h = History::default();
        let locked = EditCommand::AddOp {
            layer: Document::UNDERLAY_LAYER,
            op: DrawOp::ClearRect {
                rect: crate::PixelRect::new(0, 0, 1, 1),
            },
        };
        assert!(h.execute(&mut doc, locked).is_err());
        assert!(!h.can_undo());
    }

    #[test]
    fn replace_underlay_is_undoable() {
        let mut doc = Document::default();
        let mut h = History::default();
        let size = CanvasSize::new(8, 8).unwrap();
        let after = Underlay {
            image: Some(Arc::new(RasterImage::filled(size, Color::WHITE))),
            canvas: size,
        };
        let before = doc.underlay();
        h.execute(
            &mut doc,
            EditCommand::ReplaceUnderlay {
                before: before.clone(),
                after: after.clone(),
            },
        )
        .unwrap();
        assert_eq!(doc.underlay(), after);
        h.undo(&mut doc).unwrap();
        assert_eq!(doc.underlay(), before);
        h.redo(&mut doc).unwrap();
        assert_eq!(doc.underlay(), after);
    }

    #[test]
    fn limit_is_at_least_one() {
        assert_eq!(History::new(0).limit(), 1);
        assert!(History::default().limit() >= 50);
    }
}
