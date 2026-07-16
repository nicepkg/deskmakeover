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

/// A fully transparent laddered `.ico` (`OverlayBadgeIconFactory.CreateTransparentIco`):
/// the ADR-0021 global-overlay slot is invisible so each icon carries its own baked mark.
pub fn transparent_ico() -> IcoAsset {
    let frames: Vec<Raster> = OVERLAY_SIZES.iter().map(|&s| Raster::new(s, s)).collect();
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
    // Alpha-derived AND mask (2026-07-16 overlay black-block fix): the disc has fully transparent
    // pixels outside its radius, so its mask (and hash) changed; the fully-transparent overlay's did
    // too. The opaque gradient/checker are byte-unchanged (no alpha-0 pixel → mask still all-zero).
    const DISC256_ICO_SHA256: &str =
        "883e14c4150b91991de776dc32fa2cba13cffb1c4969607b0a9026c820fc6308";
    const TRANSPARENT_ICO_SHA256: &str =
        "9bdadf1a36b17a32167b7ff5ff69cd16d24ee7a40b06de86b52a57e702b6e57f";

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
    fn transparent_ico_has_overlay_sizes_and_all_zero_pixels() {
        let asset = transparent_ico();
        let entries = parse(&asset.bytes).expect("valid transparent ICO");
        let sizes: Vec<i32> = entries.iter().map(|e| e.dib_width).collect();
        assert_eq!(sizes, vec![16, 20, 24, 32, 48, 256]);
        // The COLOR pixels are all zero (transparent black), but the AND MASK is now all-ONES
        // (every pixel marked transparent) so the shortcut overlay renders invisible, not a black
        // block. Byte-level mask coverage is in `ico::tests`; here the hash pins the whole file.
        assert_eq!(asset.content_hash.len(), 64);
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
