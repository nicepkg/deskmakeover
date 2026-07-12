//! Native headless master bake — the webview-less render path (spec 07 §15).
//!
//! The resident process has no JS host, so background rendering can ONLY call the native
//! `dm-icon-core` kernel; this is the thin adapter from "a 256px source PNG + a resolved core
//! `Config`" to "the base64 master PNG the packaging path already consumes"
//! ([`super::package::BufferedMaster`]). The foreground webview bake and this path converge on
//! `package_masters` → `TxnDriver::apply`, so the two pipelines cannot drift beyond the kernel
//! they share.

use base64::Engine;
use dm_icon_core::compose::{ComposeDiagnostics, RenderOpts};
use dm_icon_core::config::Config;
use dm_icon_core::raster::Raster;
use dm_icon_core::render_session::RenderSession;

use crate::error::{OperationError, Result};

/// The master edge every bake targets (spec 06 §2).
const MASTER_PX: usize = 256;

/// Decodes a source PNG into the kernel's RGBA raster. Any decodable size is accepted — the
/// kernel normalizes; the extractor advertises real dimensions honestly.
pub fn raster_from_png(png: &[u8]) -> Result<Raster> {
    let img = image::load_from_memory(png)
        .map_err(|e| OperationError::InvalidPayload(format!("source png decode: {e}")))?
        .into_rgba8();
    let (w, h) = (img.width() as usize, img.height() as usize);
    let mut raster = Raster::new(w, h);
    raster.data.copy_from_slice(img.as_raw());
    Ok(raster)
}

/// FNV-1a over the source bytes — the advisory content hash `RenderSession::register` keys its
/// profile/fact caches by (native derives the real key from the raster, so collisions are safe).
pub fn source_hash(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Bakes one 256px master from a source PNG under `config`, returning the base64 PNG the
/// packaging path consumes. `field_seed` carries the hue-spread allocation (spec 07 §5: new
/// icons allocate against pinned existing seeds; `None` lets the kernel derive).
pub fn bake_master_png(
    session: &mut RenderSession,
    id: &str,
    source_png: &[u8],
    config: &Config,
    is_shortcut: bool,
    field_seed: Option<u32>,
) -> Result<String> {
    let raster = raster_from_png(source_png)?;
    session.register(id, source_hash(source_png), raster);
    session.set_look(config.clone());
    let mut diag = ComposeDiagnostics::default();
    let opts = RenderOpts { field_seed };
    let out = session
        .render(id, is_shortcut, false, MASTER_PX, &opts, &mut diag)
        .ok_or_else(|| OperationError::InvalidPayload(format!("native render produced nothing for {id}")))?;
    let mut png = Vec::new();
    {
        use image::ImageEncoder;
        image::codecs::png::PngEncoder::new(&mut png)
            .write_image(
                &out.data,
                out.width as u32,
                out.height as u32,
                image::ExtendedColorType::Rgba8,
            )
            .map_err(|e| OperationError::InvalidPayload(format!("master png encode: {e}")))?;
    }
    Ok(base64::engine::general_purpose::STANDARD.encode(png))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dm_icon_core::config::{
        Band, Distinction, FilterStyle, MarkStyle, MonoStyle, PlateFallback, Subject,
    };
    use dm_icon_core::shapes::IconShape;

    fn source_png(r: u8, g: u8, b: u8) -> Vec<u8> {
        use image::ImageEncoder;
        let img = image::RgbaImage::from_pixel(256, 256, image::Rgba([r, g, b, 255]));
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png)
            .write_image(&img, 256, 256, image::ExtendedColorType::Rgba8)
            .unwrap();
        png
    }

    fn config() -> Config {
        Config {
            shape: IconShape::Circle,
            subject: Subject::Original,
            tint: 0xFF6F5E,
            mono_style: MonoStyle::Tonal,
            plate_band: Band::Vivid,
            shortcut_shape: None,
            distinction: Distinction::None,
            mark_style: MarkStyle::Glass,
            mark_color: None,
            filter: FilterStyle::None,
            plate_color: None,
            plate_fallback: PlateFallback::Derived,
        }
    }

    #[test]
    fn bakes_a_256px_master_that_differs_from_the_raw_source() {
        let mut session = RenderSession::new();
        let src = source_png(40, 90, 200);
        let b64 = bake_master_png(&mut session, "item-1", &src, &config(), false, None).unwrap();
        let bytes = base64::engine::general_purpose::STANDARD.decode(b64).unwrap();
        let img = image::load_from_memory(&bytes).unwrap();
        assert_eq!((img.width(), img.height()), (256, 256));
        assert_ne!(bytes, src, "the styled master is not the raw source");
    }

    #[test]
    fn distinct_sources_bake_through_one_session_without_aliasing() {
        let mut session = RenderSession::new();
        let a = bake_master_png(&mut session, "a", &source_png(200, 30, 30), &config(), true, None)
            .unwrap();
        let b = bake_master_png(&mut session, "b", &source_png(30, 200, 30), &config(), true, None)
            .unwrap();
        assert_ne!(a, b, "two different sources render two different masters");
    }

    #[test]
    fn junk_source_bytes_error_cleanly() {
        let mut session = RenderSession::new();
        assert!(bake_master_png(&mut session, "x", b"not a png", &config(), false, None).is_err());
    }
}
