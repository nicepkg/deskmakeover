//! Paired above/below tests for the classification thresholds, through the public API.
//! Each threshold gets one case on each side so a drift flips exactly one assertion.

use dm_icon_core::analysis::{
    corners_symmetric, find_content_bounds, has_transparent_edges, matches_shape, max_scale_inside,
};
use dm_icon_core::color::ORIGINAL_INK_THRESHOLD;
use dm_icon_core::config::MarkStyle;
use dm_icon_core::marks::{resolve_mark, MarkContext, ADAPTIVE_THRESHOLD};
use dm_icon_core::raster::Raster;
use dm_icon_core::shapes::IconShape;

fn filled(size: usize, alpha: u8) -> Raster {
    let mut r = Raster::new(size, size);
    for p in r.data.chunks_exact_mut(4) {
        p.copy_from_slice(&[128, 128, 128, alpha]);
    }
    r
}

/// Clear a triangular notch (Manhattan depth ≤ 10) at one corner, so the diagonal
/// walk from that corner reaches depth 6 before its first solid pixel. `corner`:
/// 0 = TL, 1 = TR, 2 = BL, 3 = BR.
fn clear_corner_notch(r: &mut Raster, size: usize, corner: usize) {
    for y in 0..size {
        for x in 0..size {
            let (cx, cy) = match corner {
                0 => (x, y),
                1 => (size - 1 - x, y),
                2 => (x, size - 1 - y),
                _ => (size - 1 - x, size - 1 - y),
            };
            if cx + cy <= 10 {
                r.data[(y * size + x) * 4 + 3] = 0;
            }
        }
    }
}

#[test]
fn has_transparent_edges_pivots_at_alpha_245() {
    // A border pixel is "transparent" iff alpha < 245: 245 is opaque, 244 is not.
    assert!(!has_transparent_edges(&filled(32, 245)), "all-245 border must read opaque");
    assert!(has_transparent_edges(&filled(32, 244)), "all-244 border must read transparent");
    assert!(!has_transparent_edges(&filled(32, 255)));
}

#[test]
fn has_transparent_edges_needs_more_than_ten_percent_of_the_border() {
    // Opaque border with only a few faint pixels stays under the > total/10 gate.
    let size = 40;
    let mut r = filled(size, 255);
    for x in 0..3 {
        r.data[x * 4 + 3] = 100; // 3 faint of 4*size counted border samples
    }
    assert!(!has_transparent_edges(&r), "3 of {} faint border samples is under the 10% gate", 4 * size);
}

#[test]
fn matches_shape_accepts_the_whole_canvas_none_and_rejects_circle_for_a_square() {
    let sq = filled(64, 255); // a full solid square silhouette
    assert!(matches_shape(&sq, IconShape::None), "a full square IS the None (whole-canvas) shape");
    assert!(
        !matches_shape(&sq, IconShape::Circle),
        "a square is not a circle (IoU ≈ 0.785 < the 0.985 gate)"
    );
}

#[test]
fn max_scale_inside_is_finite_and_positive() {
    let sq = filled(64, 255);
    let b = find_content_bounds(&sq);
    let s = max_scale_inside(&sq, b, IconShape::Circle);
    assert!(s.is_finite() && s > 0.0, "max_scale_inside returned {s}");
}

#[test]
fn corners_symmetric_gates_on_diagonal_spread_not_absolute_inset() {
    // The board discriminator (analysis.ts `cornersSymmetric`) rejects dog-eared pages:
    // a full square is symmetric, one notched corner breaks it, and four EQUAL notches
    // stay symmetric — proving the gate is the corner-inset SPREAD, not the inset size.
    let sq = filled(64, 255);
    let b = find_content_bounds(&sq);
    let min_dim = 64;
    assert!(corners_symmetric(&sq, b, min_dim), "a full solid square is corner-symmetric");

    let mut dogeared = filled(64, 255);
    clear_corner_notch(&mut dogeared, 64, 0);
    let b2 = find_content_bounds(&dogeared);
    assert_eq!(b2, b, "the corner notch must not move the content bounds");
    assert!(!corners_symmetric(&dogeared, b2, min_dim), "one dog-eared corner breaks symmetry");

    let mut four = filled(64, 255);
    for corner in 0..4 {
        clear_corner_notch(&mut four, 64, corner);
    }
    let b3 = find_content_bounds(&four);
    assert_eq!(b3, b, "four equal notches keep the full bounds");
    assert!(corners_symmetric(&four, b3, min_dim), "four EQUAL notches are still symmetric");
}

#[test]
fn mark_ink_flips_at_the_0_58_adaptive_luminance_threshold() {
    // `is_light_tile(ctx)` == luminance > ADAPTIVE_THRESHOLD (strict). RingMark inks a
    // DARK ring (0x141414) on a light tile and a LIGHT ring (0xf5f5f5) on a dark tile, so
    // the fully-covered pixel is an exact readout of the crossover.
    let size = 8;
    let ink = |luminance: f64| -> [u8; 4] {
        let mark = resolve_mark(MarkStyle::Ring);
        let ctx = MarkContext {
            size,
            shape: IconShape::Circle,
            luminance,
            mark_color: None,
            tile_alpha: vec![1.0; size * size].into(),
        };
        let mut target = Raster::new(size, size);
        let mut masks = dm_icon_core::mask_cache::MaskCache::new();
        mark.render(&mut target, &vec![1.0; size * size], &ctx, &mut masks);
        [target.data[0], target.data[1], target.data[2], target.data[3]]
    };
    // Exactly at 0.58: NOT a light tile (strict >) → the dark-tile branch → light ink.
    assert_eq!(ink(ADAPTIVE_THRESHOLD), [245, 245, 245, 255], "0.58 is not a light tile");
    // One ulp above: light tile → dark ink.
    assert_eq!(ink(ADAPTIVE_THRESHOLD + 1e-9), [20, 20, 20, 255], "just above 0.58 flips to dark ink");
    // Anchors far from the boundary.
    assert_eq!(ink(0.0), [245, 245, 245, 255]);
    assert_eq!(ink(1.0), [20, 20, 20, 255]);
}

#[test]
fn original_ink_threshold_is_pinned_pending_its_consumer() {
    // ORIGINAL_INK_THRESHOLD (原彩 ink crossover, 0.66) is a FORWARD-DECLARED constant: the
    // ported Rust core has no live consumer yet (the Original subject renders through the
    // derived-field / passthrough lanes, none of which read it), so there is no branch to
    // pair-test. Pin the ported value against silent drift until its stage lands. Distinct
    // from the 0.58 mark threshold above, which IS live and boundary-tested.
    assert_eq!(ORIGINAL_INK_THRESHOLD, 0.66);
}
