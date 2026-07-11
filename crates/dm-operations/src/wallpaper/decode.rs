//! The default [`ImageDecoder`]: pure-Rust decode (the `image` crate) → true dims +
//! re-encoded PNG bytes for the compositor.
//!
//! Deliberate deviation from the plan's "WIC decode" sketch (recorded in the M6-WIRE
//! handoff): a pure-Rust decoder runs IDENTICALLY on Windows and the Mac dev host, so
//! the real production decode path is exercised by Mac unit tests and the Mac-Tauri
//! E2E instead of being one more blind-written `[WINDOWS-VERIFY]` COM seam. Covers
//! png/jpeg/bmp/gif/webp — the practical Windows wallpaper population. Exotic
//! OS-codec formats (HEIC) are a documented gap; a WIC fallback is a Windows-batch
//! upgrade if real desktops surface them.

use dm_domain::{DecodedImage, ImageDecoder, PortError, PortResult};

pub struct RustImageDecoder;

impl ImageDecoder for RustImageDecoder {
    fn decode(&self, path: &str) -> PortResult<DecodedImage> {
        if !std::path::Path::new(path).exists() {
            return Err(PortError::NotFound(path.into()));
        }
        let img = image::open(path).map_err(|e| PortError::Io(format!("decode {path}: {e}")))?;
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png)
            .write_image(&rgba, width, height, image::ExtendedColorType::Rgba8)
            .map_err(|e| PortError::Io(format!("png encode {path}: {e}")))?;
        Ok(DecodedImage { width, height, png })
    }
}

// Needed for `write_image` on the encoder.
use image::ImageEncoder;

#[cfg(test)]
mod tests {
    use super::*;

    const PNG_MAGIC: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

    /// Writes a tiny in-memory-generated image to a temp file in `format`.
    fn fixture(dir: &std::path::Path, name: &str, format: image::ImageFormat) -> String {
        let img = image::RgbaImage::from_fn(4, 3, |x, y| {
            image::Rgba([(x * 60) as u8, (y * 80) as u8, 128, 255])
        });
        let path = dir.join(name);
        image::DynamicImage::ImageRgba8(img).save_with_format(&path, format).unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn decodes_jpeg_to_true_dims_and_png_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture(dir.path(), "wall.jpg", image::ImageFormat::Jpeg);
        let out = RustImageDecoder.decode(&path).unwrap();
        assert_eq!((out.width, out.height), (4, 3));
        assert_eq!(out.png[..8], PNG_MAGIC);
    }

    #[test]
    fn decodes_png_and_bmp() {
        let dir = tempfile::tempdir().unwrap();
        for (name, fmt) in
            [("w.png", image::ImageFormat::Png), ("w.bmp", image::ImageFormat::Bmp)]
        {
            let path = fixture(dir.path(), name, fmt);
            let out = RustImageDecoder.decode(&path).unwrap();
            assert_eq!((out.width, out.height), (4, 3), "{name}");
            assert_eq!(out.png[..8], PNG_MAGIC, "{name}");
        }
    }

    #[test]
    fn missing_file_is_not_found() {
        assert!(matches!(
            RustImageDecoder.decode("/no/such/wallpaper.jpg"),
            Err(PortError::NotFound(_))
        ));
    }

    #[test]
    fn garbage_bytes_are_an_io_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("junk.jpg");
        std::fs::write(&path, b"definitely not an image").unwrap();
        assert!(matches!(
            RustImageDecoder.decode(&path.to_string_lossy()),
            Err(PortError::Io(_))
        ));
    }
}
