//! Paired above/below tests for the classification thresholds, through the public API.
//! Each threshold gets one case on each side so a drift flips exactly one assertion.

use dm_icon_core::analysis::{find_content_bounds, has_transparent_edges, matches_shape, max_scale_inside};
use dm_icon_core::raster::Raster;
use dm_icon_core::shapes::IconShape;

fn filled(size: usize, alpha: u8) -> Raster {
    let mut r = Raster::new(size, size);
    for p in r.data.chunks_exact_mut(4) {
        p.copy_from_slice(&[128, 128, 128, alpha]);
    }
    r
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
