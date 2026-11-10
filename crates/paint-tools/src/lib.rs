//! Tool logic: converts GUI-independent pointer input into draw operations
//! and selections. The UI layer feeds canvas-space pointer events into a
//! [`ToolController`] and applies the returned [`ToolEvent`]s.

mod controller;
mod settings;

pub use controller::{ToolController, ToolEvent};
pub use settings::{ToolKind, ToolSettings};
