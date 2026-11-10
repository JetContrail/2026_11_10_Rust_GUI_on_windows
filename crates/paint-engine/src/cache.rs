use paint_domain::{CanvasSize, DrawOp, PixelRect, RasterImage};

use crate::raster::{RenderOptions, op_bounds, render_op};
use crate::region::{extract_region, write_region};

/// Raster cache of one vector layer used for on-screen display.
///
/// Committed operations are rendered incrementally. The in-progress
/// operation (preview) is rendered on top; the pixels it covers are backed
/// up so the preview can be replaced or removed without re-rendering the
/// whole layer. Only one RGBA buffer of canvas size is kept.
#[derive(Debug)]
pub struct LayerCache {
    raster: RasterImage,
    preview_backup: Option<(PixelRect, Vec<u8>)>,
    dirty: Option<PixelRect>,
    options: RenderOptions,
}

impl LayerCache {
    pub fn new(canvas: CanvasSize, options: RenderOptions) -> Self {
        Self {
            raster: RasterImage::transparent(canvas),
            preview_backup: None,
            dirty: Some(PixelRect::new(0, 0, canvas.width, canvas.height)),
            options,
        }
    }

    /// Display raster (committed content plus the current preview).
    pub fn raster(&self) -> &RasterImage {
        &self.raster
    }

    pub fn canvas(&self) -> CanvasSize {
        self.raster.size()
    }

    /// Re-renders everything from `ops` (used after undo/redo/load).
    pub fn rebuild(&mut self, canvas: CanvasSize, ops: &[DrawOp]) {
        self.raster = RasterImage::transparent(canvas);
        self.preview_backup = None;
        for op in ops {
            render_op(&mut self.raster, op, self.options);
        }
        self.dirty = Some(PixelRect::new(0, 0, canvas.width, canvas.height));
    }

    /// Renders a committed operation (removes any preview first).
    pub fn commit(&mut self, op: &DrawOp) {
        self.clear_preview();
        let rect = render_op(&mut self.raster, op, self.options);
        self.mark_dirty(rect);
    }

    /// Shows `op` as the in-progress preview, replacing the previous one.
    pub fn set_preview(&mut self, op: &DrawOp) {
        self.clear_preview();
        let rect = op_bounds(op, self.raster.size());
        if rect.is_empty() {
            return;
        }
        self.preview_backup = Some((rect, extract_region(&self.raster, rect)));
        render_op(&mut self.raster, op, self.options);
        self.mark_dirty(rect);
    }

    /// Removes the preview, restoring the committed pixels.
    pub fn clear_preview(&mut self) {
        if let Some((rect, data)) = self.preview_backup.take() {
            write_region(&mut self.raster, rect, &data);
            self.mark_dirty(rect);
        }
    }

    pub fn has_preview(&self) -> bool {
        self.preview_backup.is_some()
    }

    /// Returns and resets the area changed since the last call.
    pub fn take_dirty(&mut self) -> Option<PixelRect> {
        self.dirty.take()
    }

    fn mark_dirty(&mut self, rect: PixelRect) {
        if rect.is_empty() {
            return;
        }
        self.dirty = Some(match self.dirty {
            Some(d) => d.union(rect),
            None => rect,
        });
    }
}

#[cfg(test)]
mod tests {
    use paint_domain::{Color, Point, StrokeWidth};

    use super::*;

    fn size() -> CanvasSize {
        CanvasSize::new(64, 64).unwrap()
    }

    fn dot(x: f32, y: f32) -> DrawOp {
        DrawOp::Stroke {
            color: Color::BLACK,
            width: StrokeWidth::Px8,
            points: vec![Point::new(x, y)],
        }
    }

    #[test]
    fn new_cache_is_transparent_and_fully_dirty() {
        let mut cache = LayerCache::new(size(), RenderOptions::default());
        assert!(cache.raster().pixels().iter().all(|&b| b == 0));
        assert_eq!(cache.take_dirty(), Some(PixelRect::new(0, 0, 64, 64)));
        assert_eq!(cache.take_dirty(), None);
    }

    #[test]
    fn preview_is_replaced_and_cleared() {
        let mut cache = LayerCache::new(size(), RenderOptions::default());
        cache.take_dirty();
        cache.set_preview(&dot(10.0, 10.0));
        assert_eq!(cache.raster().pixel(10, 10).unwrap().a, 255);
        cache.set_preview(&dot(40.0, 40.0));
        assert_eq!(cache.raster().pixel(10, 10).unwrap().a, 0);
        assert_eq!(cache.raster().pixel(40, 40).unwrap().a, 255);
        let dirty = cache.take_dirty().unwrap();
        assert!(dirty.contains(10, 10) && dirty.contains(40, 40));
        cache.clear_preview();
        assert!(!cache.has_preview());
        assert!(cache.raster().pixels().iter().all(|&b| b == 0));
    }

    #[test]
    fn commit_keeps_content_and_drops_preview() {
        let mut cache = LayerCache::new(size(), RenderOptions::default());
        cache.set_preview(&dot(10.0, 10.0));
        cache.commit(&dot(40.0, 40.0));
        assert_eq!(cache.raster().pixel(10, 10).unwrap().a, 0);
        assert_eq!(cache.raster().pixel(40, 40).unwrap().a, 255);
    }

    #[test]
    fn rebuild_matches_incremental_rendering() {
        let ops = vec![
            dot(10.0, 10.0),
            DrawOp::Erase {
                width: StrokeWidth::Px4,
                points: vec![Point::new(10.0, 10.0)],
            },
            dot(30.0, 30.0),
        ];
        let mut incremental = LayerCache::new(size(), RenderOptions::default());
        for op in &ops {
            incremental.commit(op);
        }
        let mut rebuilt = LayerCache::new(size(), RenderOptions::default());
        rebuilt.rebuild(size(), &ops);
        assert_eq!(incremental.raster(), rebuilt.raster());
        rebuilt.rebuild(CanvasSize::new(8, 8).unwrap(), &[]);
        assert_eq!(rebuilt.canvas(), CanvasSize::new(8, 8).unwrap());
    }
}
