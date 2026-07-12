//! Packaging the frontend's baked masters into laddered, content-addressed `.ico` assets.
//!
//! D1 boundary: the PIXELS are baked frontend-side (WASM `dm-icon-core`), sent over the bridge as
//! 256px PNG masters; Rust does the genuine platform-adjacent work of turning each master into a
//! multi-resolution `.ico` (`dm-icon-codec` ladder) the shell can reference. Rust never renders a
//! pixel — it decodes the master, resamples the ladder, and writes the ICO container.
//!
//! One item may carry two masters: `source_index` 0 is the primary, 1 is the paired empty state
//! (the Recycle Bin's empty icon). This mirrors `IconItemDto.sourceUrls` (`[0]` primary, `[1]`
//! empty) and is the single convention both sides agree on.

use base64::Engine;
use dm_icon_codec::{bake_ico, IcoAsset, Raster};

use crate::error::{OperationError, Result};

/// The primary source slot; every styleable item has one.
const PRIMARY_INDEX: u32 = 0;
/// The paired empty-state slot (the Recycle Bin's empty icon).
const EMPTY_INDEX: u32 = 1;
/// The exact bake-master size (spec 06 §2): the compositor renders every master at 256×256.
const MASTER_PX: u32 = 256;
/// A hard ceiling on a master's base64 length, so a hostile/oversized payload can never force an
/// unbounded decode. A 256×256 RGBA PNG is comfortably under this (~256 KiB raw, less compressed).
const MAX_MASTER_B64: usize = 4 * 1024 * 1024;
/// The 8-byte PNG signature — a master MUST be a PNG (not a JPEG or anything `image` would auto-detect).
const PNG_MAGIC: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// One master the frontend baked, addressed to an item + its source slot. `png_base64` is the
/// standard-alphabet base64 of a straight-alpha RGBA PNG (the 256px bake master).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferedMaster {
    pub item_id: String,
    pub source_index: u32,
    pub png_base64: String,
}

/// One item's packaged asset(s): the primary laddered ICO plus, for a two-state item, its paired
/// empty ICO. Consumed by the apply path to build the driver's `ApplyRequest`.
#[derive(Debug, Clone)]
pub struct PackagedItem {
    pub item_id: String,
    pub primary: IcoAsset,
    pub empty: Option<IcoAsset>,
}

/// Decodes + laddered-ICO-packages the buffered masters, grouped per item id (input order
/// preserved). `source_index` 0 → primary, 1 → paired empty. Fails closed on a master that does
/// not decode, an item with no primary (index 0) master, or a duplicate slot — a malformed apply
/// buffer must never silently drop or mis-pair an icon.
pub fn package_masters(masters: &[BufferedMaster]) -> Result<Vec<PackagedItem>> {
    // Group by item id, preserving first-seen order so the apply is deterministic.
    let mut order: Vec<String> = Vec::new();
    let mut primary: std::collections::HashMap<String, &BufferedMaster> = Default::default();
    let mut empty: std::collections::HashMap<String, &BufferedMaster> = Default::default();

    for m in masters {
        if !primary.contains_key(&m.item_id) && !empty.contains_key(&m.item_id) {
            order.push(m.item_id.clone());
        }
        let slot = match m.source_index {
            PRIMARY_INDEX => &mut primary,
            EMPTY_INDEX => &mut empty,
            other => {
                return Err(OperationError::Io(format!(
                    "icon master {}#{other} uses an unknown source slot (0=primary, 1=empty)",
                    m.item_id
                )));
            }
        };
        if slot.insert(m.item_id.clone(), m).is_some() {
            return Err(OperationError::Io(format!(
                "icon master {}#{} was sent twice in one apply",
                m.item_id, m.source_index
            )));
        }
    }

    let mut packaged = Vec::with_capacity(order.len());
    for id in order {
        let Some(p) = primary.get(&id) else {
            return Err(OperationError::Io(format!(
                "icon item {id} has an empty-state master but no primary (source 0)"
            )));
        };
        let primary_ico = bake_master(&p.png_base64, &id, PRIMARY_INDEX)?;
        let empty_ico = match empty.get(&id) {
            Some(e) => Some(bake_master(&e.png_base64, &id, EMPTY_INDEX)?),
            None => None,
        };
        packaged.push(PackagedItem { item_id: id, primary: primary_ico, empty: empty_ico });
    }
    Ok(packaged)
}

/// Decodes one base64 PNG master into a straight-alpha RGBA raster and bakes it into a laddered
/// content-addressed ICO. `image`'s `to_rgba8` yields exactly the straight-alpha, row-major RGBA8
/// buffer [`Raster`] holds, so the conversion is a move, not a re-encode.
fn bake_master(png_base64: &str, item_id: &str, slot: u32) -> Result<IcoAsset> {
    // Fail closed on a malformed payload (codex 2026-07-12): cap the size before decoding, require a
    // real PNG (not a JPEG `image` would happily auto-detect), and require the exact 256×256 master
    // size — a wrong-size or wrong-format master must never silently ride into the shell's icon store.
    if png_base64.len() > MAX_MASTER_B64 {
        return Err(OperationError::Io(format!(
            "icon master {item_id}#{slot}: {} base64 bytes exceeds the {MAX_MASTER_B64}-byte cap",
            png_base64.len()
        )));
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(png_base64.as_bytes())
        .map_err(|e| OperationError::Io(format!("icon master {item_id}#{slot}: base64 {e}")))?;
    if bytes.len() < PNG_MAGIC.len() || bytes[..PNG_MAGIC.len()] != PNG_MAGIC {
        return Err(OperationError::Io(format!("icon master {item_id}#{slot}: not a PNG")));
    }
    let img = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
        .map_err(|e| OperationError::Io(format!("icon master {item_id}#{slot}: decode {e}")))?;
    let (w, h) = (img.width(), img.height());
    if w != MASTER_PX || h != MASTER_PX {
        return Err(OperationError::Io(format!(
            "icon master {item_id}#{slot}: {w}×{h}, expected {MASTER_PX}×{MASTER_PX}"
        )));
    }
    let raster = Raster { width: w as usize, height: h as usize, data: img.to_rgba8().into_raw() };
    Ok(bake_ico(&raster))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A base64 PNG of a `w`×`h` solid-colour straight-alpha master.
    fn master_png(w: u32, h: u32, rgba: [u8; 4]) -> String {
        use image::ImageEncoder;
        let img = image::RgbaImage::from_pixel(w, h, image::Rgba(rgba));
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png)
            .write_image(&img, w, h, image::ExtendedColorType::Rgba8)
            .unwrap();
        base64::engine::general_purpose::STANDARD.encode(png)
    }

    fn master(id: &str, slot: u32, rgba: [u8; 4]) -> BufferedMaster {
        BufferedMaster { item_id: id.into(), source_index: slot, png_base64: master_png(256, 256, rgba) }
    }

    #[test]
    fn packages_a_single_master_into_a_laddered_ico() {
        let packaged = package_masters(&[master("app", 0, [10, 20, 30, 255])]).unwrap();
        assert_eq!(packaged.len(), 1);
        assert_eq!(packaged[0].item_id, "app");
        assert!(packaged[0].empty.is_none());
        // A real ICO container (magic 0,0,1,0) with at least one frame.
        let ico = &packaged[0].primary.bytes;
        assert_eq!(&ico[0..4], &[0, 0, 1, 0], "ICO header");
        assert_eq!(packaged[0].primary.content_hash.len(), 64, "sha-256 hex address");
    }

    #[test]
    fn pairs_the_recycle_bin_primary_and_empty() {
        let packaged = package_masters(&[
            master("bin", 0, [255, 0, 0, 255]),
            master("bin", 1, [0, 255, 0, 255]),
        ])
        .unwrap();
        assert_eq!(packaged.len(), 1, "the two slots collapse to one item");
        let item = &packaged[0];
        let empty = item.empty.as_ref().expect("index-1 master becomes the paired empty");
        // Distinct visual states → distinct content addresses.
        assert_ne!(item.primary.content_hash, empty.content_hash);
    }

    #[test]
    fn preserves_first_seen_item_order() {
        let packaged =
            package_masters(&[master("b", 0, [1, 1, 1, 255]), master("a", 0, [2, 2, 2, 255])])
                .unwrap();
        assert_eq!(packaged.iter().map(|p| p.item_id.as_str()).collect::<Vec<_>>(), vec!["b", "a"]);
    }

    #[test]
    fn identical_masters_are_content_addressed_identically() {
        let a = package_masters(&[master("x", 0, [7, 7, 7, 255])]).unwrap();
        let b = package_masters(&[master("y", 0, [7, 7, 7, 255])]).unwrap();
        assert_eq!(a[0].primary.content_hash, b[0].primary.content_hash, "same pixels → same asset");
    }

    #[test]
    fn rejects_an_empty_master_without_a_primary() {
        let err = package_masters(&[master("bin", 1, [0, 0, 0, 255])]).unwrap_err();
        assert!(matches!(err, OperationError::Io(_)), "an orphan empty must fail closed");
    }

    #[test]
    fn rejects_a_master_that_is_not_256_square() {
        for (w, h) in [(255, 256), (256, 255), (512, 512), (128, 128)] {
            let bad = BufferedMaster {
                item_id: "x".into(),
                source_index: 0,
                png_base64: master_png(w, h, [1, 2, 3, 255]),
            };
            assert!(
                package_masters(&[bad]).is_err(),
                "a {w}×{h} master must be rejected (expected 256×256)",
            );
        }
    }

    #[test]
    fn rejects_a_non_png_master_even_if_image_could_decode_it() {
        use image::ImageEncoder;
        // A valid 256×256 JPEG (RGB — JPEG has no alpha). `image` would happily auto-detect it, but
        // the master contract is PNG.
        let img = image::RgbImage::from_pixel(256, 256, image::Rgb([9, 9, 9]));
        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new(&mut jpeg)
            .write_image(&img, 256, 256, image::ExtendedColorType::Rgb8)
            .unwrap();
        let bad = BufferedMaster {
            item_id: "x".into(),
            source_index: 0,
            png_base64: base64::engine::general_purpose::STANDARD.encode(&jpeg),
        };
        assert!(package_masters(&[bad]).is_err(), "a JPEG master must be rejected (PNG only)");
    }

    #[test]
    fn rejects_an_oversized_master_before_decoding() {
        let bad = BufferedMaster {
            item_id: "x".into(),
            source_index: 0,
            png_base64: "A".repeat(MAX_MASTER_B64 + 1),
        };
        assert!(package_masters(&[bad]).is_err(), "an over-cap payload must fail before decode");
    }

    #[test]
    fn rejects_an_undecodable_master() {
        let bad = BufferedMaster { item_id: "x".into(), source_index: 0, png_base64: "not-base64!!".into() };
        assert!(package_masters(&[bad]).is_err());
        let not_png = BufferedMaster {
            item_id: "y".into(),
            source_index: 0,
            png_base64: base64::engine::general_purpose::STANDARD.encode(b"not a png"),
        };
        assert!(package_masters(&[not_png]).is_err());
    }

    #[test]
    fn rejects_an_unknown_slot_and_a_duplicate() {
        assert!(package_masters(&[master("x", 2, [0, 0, 0, 255])]).is_err(), "slot 2 is unknown");
        assert!(
            package_masters(&[master("x", 0, [0, 0, 0, 255]), master("x", 0, [1, 1, 1, 255])]).is_err(),
            "the same slot twice is a malformed buffer",
        );
    }
}
