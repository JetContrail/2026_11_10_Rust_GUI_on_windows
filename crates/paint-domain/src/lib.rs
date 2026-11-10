//! Domain model of the paint application.
//!
//! This crate has no dependencies and contains the core concepts shared by
//! every other layer: colors, geometry, canvas, layers, draw operations, the
//! document, the undo/redo history and the persistence ports (traits) that
//! outer layers implement (dependency inversion).

mod canvas;
mod color;
mod document;
mod error;
mod geometry;
mod history;
mod image;
mod layer;
mod op;
pub mod ports;
mod text;
mod width;

pub use canvas::CanvasSize;
pub use color::{Color, PALETTE};
pub use document::{Document, Underlay};
pub use error::DomainError;
pub use geometry::{PixelRect, Point};
pub use history::{DEFAULT_HISTORY_LIMIT, EditCommand, History};
pub use image::RasterImage;
pub use layer::{Layer, LayerContent, LayerId};
pub use op::DrawOp;
pub use text::TextSize;
pub use width::StrokeWidth;
