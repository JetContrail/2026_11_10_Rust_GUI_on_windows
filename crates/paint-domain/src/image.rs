use crate::{CanvasSize, Color, DomainError};

/// An RGBA8 bitmap (straight alpha, row-major, 4 bytes per pixel).
#[derive(Clone, PartialEq, Eq)]
pub struct RasterImage {
    size: CanvasSize,
    pixels: Vec<u8>,
}

impl RasterImage {
    /// Creates a fully transparent image.
    pub fn transparent(size: CanvasSize) -> Self {
        Self {
            size,
            pixels: vec![0; size.pixel_count() * 4],
        }
    }

    pub fn filled(size: CanvasSize, color: Color) -> Self {
        Self {
            size,
            pixels: color.to_array().repeat(size.pixel_count()),
        }
    }

    pub fn from_rgba(size: CanvasSize, pixels: Vec<u8>) -> Result<Self, DomainError> {
        let expected = size.pixel_count() * 4;
        if pixels.len() == expected {
            Ok(Self { size, pixels })
        } else {
            Err(DomainError::InvalidImageData {
                expected,
                actual: pixels.len(),
            })
        }
    }

    pub const fn size(&self) -> CanvasSize {
        self.size
    }

    pub const fn width(&self) -> u32 {
        self.size.width
    }

    pub const fn height(&self) -> u32 {
        self.size.height
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn pixels_mut(&mut self) -> &mut [u8] {
        &mut self.pixels
    }

    pub fn into_pixels(self) -> Vec<u8> {
        self.pixels
    }

    /// Color at `(x, y)`, `None` when out of bounds.
    pub fn pixel(&self, x: u32, y: u32) -> Option<Color> {
        let i = self.index(x, y)?;
        let p = self.pixels.get(i..i + 4)?;
        Some(Color::rgba(p[0], p[1], p[2], p[3]))
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, color: Color) {
        if let Some(i) = self.index(x, y)
            && let Some(p) = self.pixels.get_mut(i..i + 4)
        {
            p.copy_from_slice(&color.to_array());
        }
    }

    fn index(&self, x: u32, y: u32) -> Option<usize> {
        (x < self.size.width && y < self.size.height)
            .then(|| (y as usize * self.size.width as usize + x as usize) * 4)
    }
}

impl std::fmt::Debug for RasterImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RasterImage")
            .field("width", &self.size.width)
            .field("height", &self.size.height)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size(w: u32, h: u32) -> CanvasSize {
        CanvasSize::new(w, h).unwrap()
    }

    #[test]
    fn transparent_image_is_zeroed() {
        let img = RasterImage::transparent(size(3, 2));
        assert_eq!(img.pixels().len(), 24);
        assert_eq!(img.pixel(2, 1), Some(Color::TRANSPARENT));
        assert_eq!(img.pixel(3, 0), None);
    }

    #[test]
    fn set_and_get_pixel() {
        let mut img = RasterImage::filled(size(2, 2), Color::WHITE);
        img.set_pixel(1, 1, Color::rgb(1, 2, 3));
        img.set_pixel(5, 5, Color::BLACK); // out of bounds: ignored
        assert_eq!(img.pixel(1, 1), Some(Color::rgb(1, 2, 3)));
        assert_eq!(img.pixel(0, 0), Some(Color::WHITE));
    }

    #[test]
    fn from_rgba_validates_length() {
        assert!(RasterImage::from_rgba(size(2, 2), vec![0; 16]).is_ok());
        assert_eq!(
            RasterImage::from_rgba(size(2, 2), vec![0; 15]),
            Err(DomainError::InvalidImageData {
                expected: 16,
                actual: 15
            })
        );
    }
}
