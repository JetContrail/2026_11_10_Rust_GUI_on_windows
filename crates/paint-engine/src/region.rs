use paint_domain::{PixelRect, RasterImage};

/// Copies the pixels of `rect` (clamped to the image) into a tightly packed
/// RGBA buffer, e.g. for partial GPU texture updates.
pub fn extract_region(image: &RasterImage, rect: PixelRect) -> Vec<u8> {
    let rect = rect.clamp_to(image.size());
    let stride = image.width() as usize * 4;
    let row_len = rect.width as usize * 4;
    let mut out = Vec::with_capacity(row_len * rect.height as usize);
    for y in rect.y..rect.bottom() {
        let start = y as usize * stride + rect.x as usize * 4;
        out.extend_from_slice(&image.pixels()[start..start + row_len]);
    }
    out
}

/// Copies `rect` from `src` to the same place in `dst` (images of equal size).
pub fn copy_region(src: &RasterImage, dst: &mut RasterImage, rect: PixelRect) {
    if src.size() != dst.size() {
        return;
    }
    let rect = rect.clamp_to(src.size());
    let stride = src.width() as usize * 4;
    let row_len = rect.width as usize * 4;
    for y in rect.y..rect.bottom() {
        let start = y as usize * stride + rect.x as usize * 4;
        dst.pixels_mut()[start..start + row_len]
            .copy_from_slice(&src.pixels()[start..start + row_len]);
    }
}

/// Writes a tightly packed RGBA buffer produced by [`extract_region`] back.
pub(crate) fn write_region(dst: &mut RasterImage, rect: PixelRect, data: &[u8]) {
    let rect = rect.clamp_to(dst.size());
    let stride = dst.width() as usize * 4;
    let row_len = rect.width as usize * 4;
    if data.len() != row_len * rect.height as usize {
        return;
    }
    for (row, y) in (rect.y..rect.bottom()).enumerate() {
        let start = y as usize * stride + rect.x as usize * 4;
        dst.pixels_mut()[start..start + row_len]
            .copy_from_slice(&data[row * row_len..(row + 1) * row_len]);
    }
}

#[cfg(test)]
mod tests {
    use paint_domain::{CanvasSize, Color};

    use super::*;

    #[test]
    fn extract_and_write_round_trip() {
        let size = CanvasSize::new(4, 4).unwrap();
        let mut img = RasterImage::transparent(size);
        img.set_pixel(1, 2, Color::rgb(1, 2, 3));
        let rect = PixelRect::new(1, 1, 2, 2);
        let data = extract_region(&img, rect);
        assert_eq!(data.len(), 16);
        assert_eq!(&data[8..12], &[1, 2, 3, 255]);

        let mut other = RasterImage::transparent(size);
        write_region(&mut other, rect, &data);
        assert_eq!(other, img);
    }

    #[test]
    fn extract_clamps_to_image() {
        let img = RasterImage::transparent(CanvasSize::new(4, 4).unwrap());
        assert_eq!(extract_region(&img, PixelRect::new(3, 3, 10, 10)).len(), 4);
    }

    #[test]
    fn copy_region_copies_only_rect() {
        let size = CanvasSize::new(3, 3).unwrap();
        let src = RasterImage::filled(size, Color::WHITE);
        let mut dst = RasterImage::transparent(size);
        copy_region(&src, &mut dst, PixelRect::new(0, 0, 1, 3));
        assert_eq!(dst.pixel(0, 2), Some(Color::WHITE));
        assert_eq!(dst.pixel(1, 0), Some(Color::TRANSPARENT));
    }
}
