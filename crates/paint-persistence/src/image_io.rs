use std::io::Cursor;
use std::path::Path;

use image::{ImageFormat, ImageReader, RgbaImage};
use paint_domain::ports::{ImageExporter, ImageImporter, PersistenceError};
use paint_domain::{CanvasSize, RasterImage};

use crate::atomic::write_atomically;

const SUPPORTED: [ImageFormat; 3] = [ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::Bmp];

/// Decodes PNG / JPEG / BMP bytes (format detected from the content) into
/// an RGBA image. Images larger than [`CanvasSize::MAX_DIMENSION`] are
/// rejected before decoding the pixel data.
pub fn decode_image(bytes: &[u8]) -> Result<RasterImage, PersistenceError> {
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| PersistenceError::Io(e.to_string()))?;
    let format = reader
        .format()
        .filter(|f| SUPPORTED.contains(f))
        .ok_or_else(|| PersistenceError::Unsupported("PNG / JPEG / BMP のみ対応".to_owned()))?;

    let (width, height) = ImageReader::with_format(Cursor::new(bytes), format)
        .into_dimensions()
        .map_err(|e| PersistenceError::InvalidFormat(e.to_string()))?;
    let size = CanvasSize::new(width, height)
        .map_err(|_| PersistenceError::ImageTooLarge { width, height })?;

    let decoded = reader
        .decode()
        .map_err(|e| PersistenceError::InvalidFormat(e.to_string()))?;
    Ok(RasterImage::from_rgba(size, decoded.to_rgba8().into_raw())?)
}

/// Encodes an image as PNG (RGBA, alpha preserved).
pub fn encode_png(image: &RasterImage) -> Result<Vec<u8>, PersistenceError> {
    let buffer = RgbaImage::from_raw(image.width(), image.height(), image.pixels().to_vec())
        .ok_or_else(|| {
            PersistenceError::InvalidFormat("画像バッファの長さが不正です".to_owned())
        })?;
    let mut out = Cursor::new(Vec::new());
    buffer
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| PersistenceError::Io(e.to_string()))?;
    Ok(out.into_inner())
}

/// [`ImageImporter`] reading image files from disk.
#[derive(Debug, Default, Clone, Copy)]
pub struct FileImageImporter;

impl ImageImporter for FileImageImporter {
    fn import(&self, path: &Path) -> Result<RasterImage, PersistenceError> {
        decode_image(&std::fs::read(path)?)
    }
}

/// [`ImageExporter`] writing PNG files to disk.
#[derive(Debug, Default, Clone, Copy)]
pub struct PngExporter;

impl ImageExporter for PngExporter {
    fn export_png(&self, path: &Path, image: &RasterImage) -> Result<(), PersistenceError> {
        write_atomically(path, &encode_png(image)?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use paint_domain::Color;

    use super::*;

    fn sample() -> RasterImage {
        let mut img = RasterImage::transparent(CanvasSize::new(3, 2).unwrap());
        img.set_pixel(0, 0, Color::rgb(255, 0, 0));
        img.set_pixel(2, 1, Color::rgba(0, 0, 255, 100));
        img
    }

    fn encode_as(format: ImageFormat, img: &RasterImage) -> Vec<u8> {
        let rgba = RgbaImage::from_raw(img.width(), img.height(), img.pixels().to_vec()).unwrap();
        let rgb = image::DynamicImage::ImageRgba8(rgba).to_rgb8();
        let mut out = Cursor::new(Vec::new());
        rgb.write_to(&mut out, format).unwrap();
        out.into_inner()
    }

    #[test]
    fn png_round_trip_preserves_transparency() {
        let img = sample();
        let decoded = decode_image(&encode_png(&img).unwrap()).unwrap();
        assert_eq!(decoded, img);
        assert_eq!(decoded.pixel(1, 0), Some(Color::TRANSPARENT));
        assert_eq!(decoded.pixel(2, 1).unwrap().a, 100);
    }

    #[test]
    fn decodes_bmp_and_jpeg() {
        let bmp = decode_image(&encode_as(ImageFormat::Bmp, &sample())).unwrap();
        assert_eq!(bmp.pixel(0, 0), Some(Color::rgb(255, 0, 0)));
        let jpeg = decode_image(&encode_as(ImageFormat::Jpeg, &sample())).unwrap();
        assert_eq!(jpeg.size(), CanvasSize::new(3, 2).unwrap());
        assert_eq!(jpeg.pixel(1, 1).unwrap().a, 255);
    }

    #[test]
    fn rejects_unsupported_and_corrupt_data() {
        assert!(matches!(
            decode_image(b"not an image"),
            Err(PersistenceError::Unsupported(_))
        ));
        let mut png = encode_png(&sample()).unwrap();
        png.truncate(png.len() / 2);
        assert!(decode_image(&png).is_err());
        let gif = b"GIF89a\x01\x00\x01\x00\x00\x00\x00;";
        assert!(matches!(
            decode_image(gif),
            Err(PersistenceError::Unsupported(_))
        ));
    }

    #[test]
    fn rejects_too_large_images_without_decoding() {
        let big = RgbaImage::new(CanvasSize::MAX_DIMENSION + 1, 1);
        let mut out = Cursor::new(Vec::new());
        big.write_to(&mut out, ImageFormat::Png).unwrap();
        assert!(matches!(
            decode_image(&out.into_inner()),
            Err(PersistenceError::ImageTooLarge { width: 4097, .. })
        ));
    }

    #[test]
    fn file_adapters_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.png");
        PngExporter.export_png(&path, &sample()).unwrap();
        assert_eq!(FileImageImporter.import(&path).unwrap(), sample());
        assert!(matches!(
            FileImageImporter.import(&dir.path().join("missing.png")),
            Err(PersistenceError::Io(_))
        ));
    }
}
