//! Persistence adapters implementing the ports of `paint_domain::ports`.
//!
//! - [`FileImageImporter`]: PNG / JPEG / BMP decoding for the underlay
//! - [`PngExporter`]: PNG writing with alpha channel (transparent PNG)
//! - [`JsonProjectStore`]: `.paint` project files (see `docs/FileFormat.md`)

mod atomic;
mod image_io;
mod project;
mod system_fonts;

pub use image_io::{FileImageImporter, PngExporter, decode_image, encode_png};
pub use project::{
    FORMAT_ID, FORMAT_VERSION, JsonProjectStore, PROJECT_EXTENSION, decode_project, encode_project,
};
pub use system_fonts::SystemFontSource;
