//! Drawing engine.
//!
//! Rasterizes [`DrawOp`](paint_domain::DrawOp)s into RGBA buffers with a
//! round, optionally anti-aliased brush, keeps per-layer raster caches for
//! fast incremental display updates and composes layers for export.

mod blend;
mod cache;
mod compose;
mod raster;
mod region;

pub use cache::LayerCache;
pub use compose::{compose_visible, render_vector_layer};
pub use raster::{RenderOptions, op_bounds, render_op};
pub use region::{copy_region, extract_region};
