//! Application layer (ViewModel). Owns the document, undo/redo history,
//! tool state and display caches; the UI only forwards input and renders
//! the state exposed here. Persistence is injected through the
//! `paint_domain::ports` traits (dependency inversion), so this crate does
//! not depend on file formats or the GUI toolkit.

mod services;
mod viewmodel;

pub use services::Services;
pub use viewmodel::{AppError, ExportMode, PaintViewModel};
