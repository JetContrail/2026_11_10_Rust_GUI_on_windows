use paint_domain::{CanvasSize, Document, DrawOp, LayerContent, RasterImage};

use crate::blend::blend_over;
use crate::raster::{RenderOptions, render_op};

/// Renders the operations of a vector layer onto a transparent raster.
pub fn render_vector_layer(
    canvas: CanvasSize,
    ops: &[DrawOp],
    options: RenderOptions,
) -> RasterImage {
    let mut image = RasterImage::transparent(canvas);
    for op in ops {
        render_op(&mut image, op, options);
    }
    image
}

/// Composes the visible layers selected by `include` (bottom to top) onto a
/// transparent canvas. Areas without content stay transparent, so the
/// result can be written as a transparent PNG.
pub fn compose_visible(
    doc: &Document,
    options: RenderOptions,
    include: impl Fn(&paint_domain::Layer) -> bool,
) -> RasterImage {
    let canvas = doc.canvas();
    let mut out = RasterImage::transparent(canvas);
    for layer in doc.layers().iter().filter(|l| l.visible && include(l)) {
        match &layer.content {
            LayerContent::Raster(Some(image)) => blend_image(&mut out, image),
            LayerContent::Raster(None) => {}
            LayerContent::Vector(ops) => {
                let rendered = render_vector_layer(canvas, ops, options);
                blend_image(&mut out, &rendered);
            }
        }
    }
    out
}

fn blend_image(dst: &mut RasterImage, src: &RasterImage) {
    if dst.size() != src.size() {
        return;
    }
    for (d, s) in dst
        .pixels_mut()
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(src.pixels().as_chunks::<4>().0)
    {
        if s[3] > 0 {
            blend_over(d, *s, 1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use paint_domain::{Color, Point, StrokeWidth, Underlay};

    use super::*;

    fn doc_with_underlay() -> Document {
        let size = CanvasSize::new(20, 20).unwrap();
        let mut doc = Document::new(size);
        doc.replace_underlay(Underlay {
            image: Some(Arc::new(RasterImage::filled(size, Color::WHITE))),
            canvas: size,
        })
        .unwrap();
        doc.push_op(
            Document::DRAWING_LAYER,
            DrawOp::Stroke {
                color: Color::rgb(0, 0, 255),
                width: StrokeWidth::Px4,
                points: vec![Point::new(10.0, 10.0)],
            },
        )
        .unwrap();
        doc
    }

    #[test]
    fn composes_drawing_over_underlay() {
        let doc = doc_with_underlay();
        let img = compose_visible(&doc, RenderOptions::default(), |_| true);
        assert_eq!(img.pixel(10, 10), Some(Color::rgb(0, 0, 255)));
        assert_eq!(img.pixel(0, 0), Some(Color::WHITE));
    }

    #[test]
    fn hidden_or_excluded_layers_leave_transparency() {
        let mut doc = doc_with_underlay();
        let drawing_only = compose_visible(&doc, RenderOptions::default(), |l| !l.locked);
        assert_eq!(drawing_only.pixel(0, 0), Some(Color::TRANSPARENT));
        assert_eq!(drawing_only.pixel(10, 10), Some(Color::rgb(0, 0, 255)));

        doc.set_layer_visible(Document::DRAWING_LAYER, false)
            .unwrap();
        let underlay_only = compose_visible(&doc, RenderOptions::default(), |_| true);
        assert_eq!(underlay_only.pixel(10, 10), Some(Color::WHITE));
    }

    #[test]
    fn render_vector_layer_applies_ops_in_order() {
        let size = CanvasSize::new(10, 10).unwrap();
        let ops = vec![
            DrawOp::Stroke {
                color: Color::BLACK,
                width: StrokeWidth::Px32,
                points: vec![Point::new(5.0, 5.0)],
            },
            DrawOp::ClearRect {
                rect: paint_domain::PixelRect::new(0, 0, 5, 10),
            },
        ];
        let img = render_vector_layer(size, &ops, RenderOptions::default());
        assert_eq!(img.pixel(2, 2), Some(Color::TRANSPARENT));
        assert_eq!(img.pixel(7, 7), Some(Color::BLACK));
    }
}
