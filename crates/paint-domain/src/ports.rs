//! Persistence ports implemented by outer layers (dependency inversion).
//!
//! The application layer only knows these traits; the concrete
//! implementations live in `paint-persistence` and are injected by the
//! executable.

use std::fmt;
use std::path::Path;

use crate::{Document, DomainError, RasterImage};

/// Errors of persistence adapters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersistenceError {
    Io(String),
    /// The file content is not valid for the expected format.
    InvalidFormat(String),
    /// Unsupported image format or file version.
    Unsupported(String),
    ImageTooLarge {
        width: u32,
        height: u32,
    },
    Domain(DomainError),
}

impl fmt::Display for PersistenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "ファイルの読み書きに失敗しました: {e}"),
            Self::InvalidFormat(e) => write!(f, "ファイルの形式が不正です: {e}"),
            Self::Unsupported(e) => write!(f, "未対応の形式です: {e}"),
            Self::ImageTooLarge { width, height } => write!(
                f,
                "画像サイズ {width}×{height} は上限 {max}×{max} を超えています",
                max = crate::CanvasSize::MAX_DIMENSION
            ),
            Self::Domain(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for PersistenceError {}

impl From<DomainError> for PersistenceError {
    fn from(e: DomainError) -> Self {
        Self::Domain(e)
    }
}

impl From<std::io::Error> for PersistenceError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

/// Saves and loads `.paint` project files.
pub trait ProjectStore: fmt::Debug {
    fn save(&self, path: &Path, document: &Document) -> Result<(), PersistenceError>;
    fn load(&self, path: &Path) -> Result<Document, PersistenceError>;
}

/// Decodes image files (PNG / JPEG / BMP) used as underlay.
pub trait ImageImporter: fmt::Debug {
    fn import(&self, path: &Path) -> Result<RasterImage, PersistenceError>;
}

/// Writes images to disk.
pub trait ImageExporter: fmt::Debug {
    /// Writes `image` as PNG (alpha channel preserved).
    fn export_png(&self, path: &Path, image: &RasterImage) -> Result<(), PersistenceError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_messages_are_human_readable() {
        let e = PersistenceError::ImageTooLarge {
            width: 5000,
            height: 10,
        };
        assert!(e.to_string().contains("5000"));
        let io: PersistenceError = std::io::Error::other("boom").into();
        assert!(matches!(io, PersistenceError::Io(_)));
        let d: PersistenceError = DomainError::NoUnderlayLayer.into();
        assert_eq!(d.to_string(), DomainError::NoUnderlayLayer.to_string());
    }
}
