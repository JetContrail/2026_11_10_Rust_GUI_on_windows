use std::fmt;

use crate::LayerId;

/// Errors raised when an operation would violate a domain rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    InvalidCanvasSize { width: u32, height: u32 },
    InvalidImageData { expected: usize, actual: usize },
    LayerNotFound(LayerId),
    LayerLocked(LayerId),
    NotAVectorLayer(LayerId),
    NoUnderlayLayer,
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCanvasSize { width, height } => write!(
                f,
                "キャンバスサイズ {width}×{height} は範囲外です (1〜{} px)",
                crate::CanvasSize::MAX_DIMENSION
            ),
            Self::InvalidImageData { expected, actual } => write!(
                f,
                "画像データの長さが不正です (期待値 {expected} バイト, 実際 {actual} バイト)"
            ),
            Self::LayerNotFound(id) => write!(f, "レイヤー {} が見つかりません", id.0),
            Self::LayerLocked(id) => write!(f, "レイヤー {} は編集禁止です", id.0),
            Self::NotAVectorLayer(id) => {
                write!(f, "レイヤー {} は描画レイヤーではありません", id.0)
            }
            Self::NoUnderlayLayer => write!(f, "下絵レイヤーがありません"),
        }
    }
}

impl std::error::Error for DomainError {}
