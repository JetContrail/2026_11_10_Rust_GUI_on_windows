use std::sync::Arc;

use crate::{DrawOp, RasterImage};

/// Stable identifier of a layer inside a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LayerId(pub u32);

/// What a layer contains.
#[derive(Debug, Clone, PartialEq)]
pub enum LayerContent {
    /// Bitmap layer (used for the underlay image). `None` = empty.
    Raster(Option<Arc<RasterImage>>),
    /// Drawing layer: ordered draw operations.
    Vector(Vec<DrawOp>),
}

/// One layer of the document. Layers are stored bottom to top.
#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    pub visible: bool,
    /// Locked layers cannot be drawn on.
    pub locked: bool,
    pub content: LayerContent,
}

impl Layer {
    pub fn underlay(id: LayerId) -> Self {
        Self {
            id,
            name: "下絵".to_owned(),
            visible: true,
            locked: true,
            content: LayerContent::Raster(None),
        }
    }

    pub fn drawing(id: LayerId) -> Self {
        Self {
            id,
            name: "描画".to_owned(),
            visible: true,
            locked: false,
            content: LayerContent::Vector(Vec::new()),
        }
    }

    pub fn ops(&self) -> Option<&[DrawOp]> {
        match &self.content {
            LayerContent::Vector(ops) => Some(ops),
            LayerContent::Raster(_) => None,
        }
    }

    pub fn image(&self) -> Option<&Arc<RasterImage>> {
        match &self.content {
            LayerContent::Raster(image) => image.as_ref(),
            LayerContent::Vector(_) => None,
        }
    }
}
