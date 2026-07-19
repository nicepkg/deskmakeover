//! The icon size ladder and the two consumer entry points (baked asset + transparent
//! overlay). Ports `GeneratedIconStore.Save` and `OverlayBadgeIconFactory`.
//!
//! The resampler is **reused** from `dm_icon_core::sampling::downscale` — the codec never
//! re-implements area averaging (DRY iron rule). That keeps the baked ICO pixels equal to
//! the on-screen preview: both run the M5-certified core resampler (v1's single truth
//! source, ADR-0019), not the C# `IconResampler` whose `Math.Round` ties-to-even would
//! diverge from the core's `js_round` at .5-tie bytes.

use dm_icon_core::raster::Raster;
use dm_icon_core::sampling::downscale;

use crate::hash::{write_ico_asset, IcoAsset};

/// Generated-icon size ladder, largest first (`GeneratedIconStore.IconSizes`).
pub const LADDER_SIZES: [usize; 6] = [256, 48, 32, 24, 20, 16];

/// Overlay size ladder, smallest first (`OverlayBadgeIconFactory.Sizes`) — the ADR-0021
/// transparent global-overlay slot ships every one at true 32-bit alpha.
pub const OVERLAY_SIZES: [usize; 6] = [16, 20, 24, 32, 48, 256];

/// Build the size ladder from a source render: every ladder size that fits BOTH source
/// dimensions (`<= source.width` AND `<= source.height`, so a non-square source never
/// upsamples an axis), each a linear-light area-average downscale of the source
/// (`GeneratedIconStore.Save` → `IconResampler.Downscale`). Falls back to `[source]` when
/// the source is smaller than every ladder rung.
pub fn resample_ladder(source: &Raster) -> Vec<Raster> {
    // A zero-dimension source produces a frame the codec's own `parse` rejects
    // (dib_width <= 0); refuse it at the source instead of baking an invalid asset.
    assert!(source.width > 0 && source.height > 0, "cannot bake a zero-dimension source");
    let mut frames: Vec<Raster> = LADDER_SIZES
        .iter()
        .copied()
        // Both axes must fit: a rung <= width but > height on a non-square source
        // would drive `downscale`'s scale_y through the box-average path as an
        // UPSAMPLE, softening the y-axis. Keep only rungs that truly downscale. (ICON-4)
        .filter(|&size| size <= source.width && size <= source.height)
        .map(|size| downscale(source, size))
        .collect();
    if frames.is_empty() {
        frames.push(source.clone());
    }
    frames
}

/// Bake a source render into a laddered `.ico` + its content hash — the asset the M34
/// transaction driver stores as an `AssetRef`. The paired Recycle-Bin `<asset>-empty.ico`
/// is simply a second `bake_ico` of the empty-state source (dm-windows derives the path).
pub fn bake_ico(source: &Raster) -> IcoAsset {
    write_ico_asset(&resample_ladder(source))
}

/// The minimum alpha every transparent-overlay pixel carries. ⛔ Never ship an all-zero alpha
/// plane into a shell-persisted surface: Windows' "no nonzero alpha byte ⇒ legacy no-alpha
/// icon" heuristic (Explorer's icon-cache deserialize, image-list adds) reclassifies such a
/// bitmap as fully OPAQUE, and its zero RGB then paints as a solid BLACK block over every
/// shortcut (the 2026-07-19 incident's overlay half). Alpha 1/255 ≈ 0.4% is imperceptible on
/// any background yet defeats the heuristic in every path, live and cached.
const OVERLAY_MIN_ALPHA: u8 = 1;

/// The visually transparent laddered `.ico` (`OverlayBadgeIconFactory.CreateTransparentIco`):
/// the ADR-0021 global-overlay slot is invisible so each icon carries its own baked mark.
/// Pixels are (0,0,0,alpha=1), not (0,0,0,0) — see [`OVERLAY_MIN_ALPHA`].
pub fn transparent_ico() -> IcoAsset {
    let frames: Vec<Raster> = OVERLAY_SIZES
        .iter()
        .map(|&s| {
            let mut r = Raster::new(s, s);
            for px in r.data.chunks_exact_mut(4) {
                px[3] = OVERLAY_MIN_ALPHA;
            }
            r
        })
        .collect();
    write_ico_asset(&frames)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ico::parse;

    // Pinned content addresses — see `print_pinned_hashes` to recapture.
    const GRADIENT256_ICO_SHA256: &str =
        "b8c580a2f7bc4509ccfd44d9bae625df37a59e8f663017166419d607768c681d";
    const CHECKER256_ICO_SHA256: &str =
        "1401770a5bac81ef40340b399083a6a56f1b67396db46b8958d563a9a9950ff2";
    // 2026-07-19 black-icon fix: the AND mask is all-zero again for every profile (the alpha-
    // derived mask poisoned the Explorer icon-cache round trip), and the transparent overlay
    // carries alpha=1. The disc reverts to its pre-mask-experiment bytes; the transparent
    // overlay's bytes (and hash — the overlay install signature) change, which deliberately
    // triggers a one-time overlay reinstall on the next apply (icon_host reinstalls on any
    // signature change, self-healing customer machines).
    const DISC256_ICO_SHA256: &str =
        "7ddabb467f7188490b7e733608018b6c1def6b4717c211e23baf58010b89df70";
    const TRANSPARENT_ICO_SHA256: &str =
        "67565c196340470df1a134980ee4099bf5552556ba7d66d8d6bc7c8f17e14ee1";

    fn gradient(size: usize) -> Raster {
        // A deterministic, non-uniform source that exercises every channel + the alpha
        // edge, so the ladder downscale is a real average (not a trivial solid).
        let mut r = Raster::new(size, size);
        for y in 0..size {
            for x in 0..size {
                let i = (y * size + x) * 4;
                r.data[i] = (x * 255 / size.max(1)) as u8;
                r.data[i + 1] = (y * 255 / size.max(1)) as u8;
                r.data[i + 2] = ((x + y) * 255 / (2 * size.max(1))) as u8;
                r.data[i + 3] = if (x + y) % 3 == 0 { 255 } else { 96 };
            }
        }
        r
    }

    #[test]
    fn ladder_reuses_the_core_downscale_exactly() {
        let src = gradient(256);
        let frames = resample_ladder(&src);
        let sizes: Vec<usize> = frames.iter().map(|f| f.width).collect();
        assert_eq!(sizes, LADDER_SIZES.to_vec());
        // Each frame must be byte-identical to the core resampler at that size — proving
        // the reuse (no divergent second implementation).
        for (frame, &size) in frames.iter().zip(LADDER_SIZES.iter()) {
            assert_eq!(frame.data, downscale(&src, size).data);
            assert_eq!((frame.width, frame.height), (size, size));
        }
    }

    #[test]
    fn ladder_filters_sizes_larger_than_a_small_source() {
        let src = gradient(24);
        let sizes: Vec<usize> = resample_ladder(&src).iter().map(|f| f.width).collect();
        assert_eq!(sizes, vec![24, 20, 16]); // 256/48/32 dropped
    }

    #[test]
    fn ladder_falls_back_to_the_source_when_below_every_rung() {
        let src = gradient(12); // smaller than 16
        let frames = resample_ladder(&src);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].data, src.data);
    }

    #[test]
    fn bake_ico_is_structurally_valid_and_deterministic() {
        let src = gradient(256);
        let a = bake_ico(&src);
        let b = bake_ico(&src);
        assert_eq!(a.content_hash, b.content_hash);
        let entries = parse(&a.bytes).expect("valid baked ICO");
        let sizes: Vec<i32> = entries.iter().map(|e| e.dib_width).collect();
        assert_eq!(sizes, vec![256, 48, 32, 24, 20, 16]);
    }

    #[test]
    fn transparent_ico_is_invisible_but_never_alpha_zero() {
        // REGRESSION (2026-07-19 incident, overlay half): an all-zero alpha plane gets
        // reclassified as a legacy no-alpha icon by Explorer's cache/image-list heuristic and
        // comes back as an OPAQUE BLACK arrow stamped over every shortcut. Every pixel must be
        // (0,0,0,1): visually nothing, but alpha-carrying in every consumer path. The AND mask
        // stays all-zero like every other frame (ico::and_mask_is_always_all_zero).
        let asset = transparent_ico();
        let entries = parse(&asset.bytes).expect("valid transparent ICO");
        let sizes: Vec<i32> = entries.iter().map(|e| e.dib_width).collect();
        assert_eq!(sizes, vec![16, 20, 24, 32, 48, 256]);
        for (e, &size) in entries.iter().zip(OVERLAY_SIZES.iter()) {
            let xor_start = e.image_offset as usize + 40;
            let xor = &asset.bytes[xor_start..xor_start + size * size * 4];
            // BGRA: colour bytes zero, alpha byte exactly OVERLAY_MIN_ALPHA — no pixel alpha-0.
            for px in xor.chunks_exact(4) {
                assert_eq!(px, [0, 0, 0, OVERLAY_MIN_ALPHA]);
            }
            let mask_len = ((size + 31) / 32) * 4 * size;
            let mask_start = xor_start + size * size * 4;
            assert!(
                asset.bytes[mask_start..mask_start + mask_len].iter().all(|&b| b == 0),
                "{size}px overlay frame must keep the all-zero AND mask"
            );
        }
    }

    fn checkerboard(size: usize) -> Raster {
        let mut r = Raster::new(size, size);
        for y in 0..size {
            for x in 0..size {
                let i = (y * size + x) * 4;
                let on = ((x / 8) + (y / 8)) % 2 == 0;
                let v = if on { 240 } else { 15 };
                r.data[i] = v;
                r.data[i + 1] = v;
                r.data[i + 2] = v;
                r.data[i + 3] = 255;
            }
        }
        r
    }

    fn alpha_disc(size: usize) -> Raster {
        let mut r = Raster::new(size, size);
        let c = size as f64 / 2.0;
        let radius = size as f64 * 0.45;
        for y in 0..size {
            for x in 0..size {
                let i = (y * size + x) * 4;
                // libm::sqrt + explicit d*d (not powi/std sqrt) keeps the fixture inside the
                // determinism doctrine; both are IEEE-correctly-rounded, so the pinned hash
                // is unaffected.
                let dx = x as f64 - c;
                let dy = y as f64 - c;
                let d = libm::sqrt(dx * dx + dy * dy);
                let a = if d <= radius { 255 } else { 0 };
                r.data[i] = 20;
                r.data[i + 1] = 130;
                r.data[i + 2] = 220;
                r.data[i + 3] = a;
            }
        }
        r
    }

    #[test]
    #[ignore = "recapture helper: cargo test -p dm-icon-codec print_pinned -- --ignored --nocapture"]
    fn print_pinned_hashes() {
        // Prints the anchors pinned above; run only when intentionally recapturing them.
        println!("gradient256={}", bake_ico(&gradient(256)).content_hash);
        println!("checker256={}", bake_ico(&checkerboard(256)).content_hash);
        println!("disc256={}", bake_ico(&alpha_disc(256)).content_hash);
        println!("transparent={}", transparent_ico().content_hash);
    }

    #[test]
    fn pinned_bake_hashes_never_drift() {
        // Committed content-address anchors over deterministic real bakes: any accidental
        // byte change in the container OR the reused core resampler flips a hash here. The
        // ICO of these fixed sources is platform-independent (libm, f64, no SIMD).
        assert_eq!(bake_ico(&gradient(256)).content_hash, GRADIENT256_ICO_SHA256);
        assert_eq!(bake_ico(&checkerboard(256)).content_hash, CHECKER256_ICO_SHA256);
        assert_eq!(bake_ico(&alpha_disc(256)).content_hash, DISC256_ICO_SHA256);
        assert_eq!(transparent_ico().content_hash, TRANSPARENT_ICO_SHA256);
    }

    #[test]
    #[should_panic(expected = "zero-dimension source")]
    fn baking_a_zero_dimension_source_is_rejected() {
        // A 0×0 render would otherwise ladder-fall-back to a single 0×0 frame whose DIB the
        // codec's own parse rejects; refuse it up front.
        bake_ico(&Raster::new(0, 0));
    }

    #[test]
    fn a_full_and_empty_recyclebin_pair_have_distinct_hashes() {
        // The Recycle-Bin consumer renders two states; distinct pixels → distinct assets.
        let full = gradient(256);
        let mut empty = gradient(256);
        empty.data.iter_mut().for_each(|v| *v = v.saturating_sub(20));
        assert_ne!(bake_ico(&full).content_hash, bake_ico(&empty).content_hash);
    }
}
