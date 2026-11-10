use std::sync::Arc;

use crate::{CanvasSize, DomainError, DrawOp, Layer, LayerContent, LayerId, RasterImage};

/// The underlay state: background image and the canvas size it implies.
#[derive(Debug, Clone, PartialEq)]
pub struct Underlay {
    pub image: Option<Arc<RasterImage>>,
    pub canvas: CanvasSize,
}

/// A paint document: canvas size and layers (bottom to top).
///
/// Invariants (checked by [`Document::from_parts`]):
/// - exactly one raster (underlay) layer and at least one vector layer exist
/// - layer ids are unique
/// - an underlay image always has the canvas size
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    canvas: CanvasSize,
    layers: Vec<Layer>,
}

impl Document {
    pub const UNDERLAY_LAYER: LayerId = LayerId(0);
    pub const DRAWING_LAYER: LayerId = LayerId(1);

    /// New document with an empty underlay layer and an empty drawing layer.
    pub fn new(canvas: CanvasSize) -> Self {
        Self {
            canvas,
            layers: vec![
                Layer::underlay(Self::UNDERLAY_LAYER),
                Layer::drawing(Self::DRAWING_LAYER),
            ],
        }
    }

    /// Builds a document from loaded parts, validating the invariants.
    pub fn from_parts(canvas: CanvasSize, layers: Vec<Layer>) -> Result<Self, DomainError> {
        let mut ids = std::collections::HashSet::new();
        let mut rasters = 0;
        let mut vectors = 0;
        for layer in &layers {
            if !ids.insert(layer.id) {
                return Err(DomainError::LayerNotFound(layer.id));
            }
            match &layer.content {
                LayerContent::Raster(image) => {
                    rasters += 1;
                    if let Some(image) = image
                        && image.size() != canvas
                    {
                        return Err(DomainError::InvalidCanvasSize {
                            width: image.width(),
                            height: image.height(),
                        });
                    }
                }
                LayerContent::Vector(_) => vectors += 1,
            }
        }
        if rasters != 1 {
            return Err(DomainError::NoUnderlayLayer);
        }
        if vectors == 0 {
            return Err(DomainError::NotAVectorLayer(LayerId(u32::MAX)));
        }
        Ok(Self { canvas, layers })
    }

    pub const fn canvas(&self) -> CanvasSize {
        self.canvas
    }

    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    pub fn layer(&self, id: LayerId) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }

    fn layer_mut(&mut self, id: LayerId) -> Result<&mut Layer, DomainError> {
        self.layers
            .iter_mut()
            .find(|l| l.id == id)
            .ok_or(DomainError::LayerNotFound(id))
    }

    /// The (only) raster layer holding the underlay image.
    pub fn underlay_layer(&self) -> Option<&Layer> {
        self.layers
            .iter()
            .find(|l| matches!(l.content, LayerContent::Raster(_)))
    }

    /// The topmost editable vector layer (the default drawing target).
    pub fn drawing_layer(&self) -> Option<&Layer> {
        self.layers
            .iter()
            .rev()
            .find(|l| !l.locked && matches!(l.content, LayerContent::Vector(_)))
    }

    pub fn ops(&self, id: LayerId) -> Option<&[DrawOp]> {
        self.layer(id).and_then(Layer::ops)
    }

    pub fn set_layer_visible(&mut self, id: LayerId, visible: bool) -> Result<(), DomainError> {
        self.layer_mut(id)?.visible = visible;
        Ok(())
    }

    /// Appends a draw operation. Fails for locked or non-vector layers.
    pub fn push_op(&mut self, id: LayerId, op: DrawOp) -> Result<(), DomainError> {
        let layer = self.layer_mut(id)?;
        if layer.locked {
            return Err(DomainError::LayerLocked(id));
        }
        match &mut layer.content {
            LayerContent::Vector(ops) => {
                ops.push(op);
                Ok(())
            }
            LayerContent::Raster(_) => Err(DomainError::NotAVectorLayer(id)),
        }
    }

    /// Removes and returns the last draw operation of a vector layer.
    pub fn pop_op(&mut self, id: LayerId) -> Result<Option<DrawOp>, DomainError> {
        match &mut self.layer_mut(id)?.content {
            LayerContent::Vector(ops) => Ok(ops.pop()),
            LayerContent::Raster(_) => Err(DomainError::NotAVectorLayer(id)),
        }
    }

    pub fn underlay(&self) -> Underlay {
        Underlay {
            image: self.underlay_layer().and_then(|l| l.image().cloned()),
            canvas: self.canvas,
        }
    }

    /// Replaces the underlay image and canvas size, returning the previous
    /// state. Loading a background image is allowed even though the
    /// underlay layer is locked against drawing.
    pub fn replace_underlay(&mut self, underlay: Underlay) -> Result<Underlay, DomainError> {
        if let Some(image) = &underlay.image
            && image.size() != underlay.canvas
        {
            return Err(DomainError::InvalidCanvasSize {
                width: image.width(),
                height: image.height(),
            });
        }
        let previous = self.underlay();
        let layer = self
            .layers
            .iter_mut()
            .find(|l| matches!(l.content, LayerContent::Raster(_)))
            .ok_or(DomainError::NoUnderlayLayer)?;
        layer.content = LayerContent::Raster(underlay.image);
        self.canvas = underlay.canvas;
        Ok(previous)
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new(CanvasSize::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Color, Point, StrokeWidth};

    fn dot() -> DrawOp {
        DrawOp::Stroke {
            color: Color::BLACK,
            width: StrokeWidth::Px1,
            points: vec![Point::new(1.0, 1.0)],
        }
    }

    #[test]
    fn new_document_has_underlay_and_drawing_layers() {
        let doc = Document::default();
        assert_eq!(doc.layers().len(), 2);
        let underlay = doc.underlay_layer().unwrap();
        assert!(underlay.locked);
        assert_eq!(underlay.id, Document::UNDERLAY_LAYER);
        let drawing = doc.drawing_layer().unwrap();
        assert!(!drawing.locked);
        assert_eq!(drawing.id, Document::DRAWING_LAYER);
    }

    #[test]
    fn underlay_layer_is_not_editable() {
        let mut doc = Document::default();
        assert_eq!(
            doc.push_op(Document::UNDERLAY_LAYER, dot()),
            Err(DomainError::LayerLocked(Document::UNDERLAY_LAYER))
        );
    }

    #[test]
    fn push_and_pop_ops() {
        let mut doc = Document::default();
        doc.push_op(Document::DRAWING_LAYER, dot()).unwrap();
        assert_eq!(doc.ops(Document::DRAWING_LAYER).unwrap().len(), 1);
        assert_eq!(doc.pop_op(Document::DRAWING_LAYER).unwrap(), Some(dot()));
        assert_eq!(doc.pop_op(Document::DRAWING_LAYER).unwrap(), None);
        assert!(doc.push_op(LayerId(9), dot()).is_err());
    }

    #[test]
    fn replace_underlay_changes_canvas_and_returns_previous() {
        let mut doc = Document::default();
        let size = CanvasSize::new(4, 3).unwrap();
        let image = Arc::new(RasterImage::filled(size, Color::WHITE));
        let prev = doc
            .replace_underlay(Underlay {
                image: Some(image.clone()),
                canvas: size,
            })
            .unwrap();
        assert_eq!(prev.canvas, CanvasSize::default());
        assert!(prev.image.is_none());
        assert_eq!(doc.canvas(), size);
        assert_eq!(doc.underlay().image, Some(image));
    }

    #[test]
    fn replace_underlay_rejects_size_mismatch() {
        let mut doc = Document::default();
        let image = Arc::new(RasterImage::transparent(CanvasSize::new(4, 3).unwrap()));
        assert!(
            doc.replace_underlay(Underlay {
                image: Some(image),
                canvas: CanvasSize::new(5, 3).unwrap(),
            })
            .is_err()
        );
        assert_eq!(doc.canvas(), CanvasSize::default());
    }

    #[test]
    fn from_parts_validates() {
        let canvas = CanvasSize::default();
        let ok = Document::from_parts(
            canvas,
            vec![Layer::underlay(LayerId(0)), Layer::drawing(LayerId(1))],
        );
        assert_eq!(ok.unwrap(), Document::new(canvas));
        assert!(Document::from_parts(canvas, vec![Layer::drawing(LayerId(1))]).is_err());
        assert!(Document::from_parts(canvas, vec![Layer::underlay(LayerId(0))]).is_err());
        assert!(
            Document::from_parts(
                canvas,
                vec![Layer::underlay(LayerId(0)), Layer::drawing(LayerId(0))]
            )
            .is_err()
        );
    }

    #[test]
    fn visibility_toggle() {
        let mut doc = Document::default();
        doc.set_layer_visible(Document::UNDERLAY_LAYER, false)
            .unwrap();
        assert!(!doc.layer(Document::UNDERLAY_LAYER).unwrap().visible);
    }
}
