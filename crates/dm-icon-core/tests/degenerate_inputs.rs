//! Degenerate / tiny-input robustness — the review-confirmed P2 panics, each pinned
//! red-then-green through the PUBLIC API. Every case panicked before its guard landed;
//! the oracle corpus is all well-formed 256² squares, so these guards only fire outside
//! it and the byte-for-byte certification is unchanged.

use dm_icon_core::analysis::{has_transparent_edges, try_detect_background};
use dm_icon_core::compose::{render_tile, ComposeDiagnostics, ComposeLane, RenderOpts};
use dm_icon_core::config::{
    Band, Config, Distinction, FilterStyle, MarkStyle, MonoStyle, PlateFallback, Subject,
};
use dm_icon_core::filters::apply_filter;
use dm_icon_core::raster::Raster;
use dm_icon_core::shapes::IconShape;

fn opaque(w: usize, h: usize) -> Raster {
    let mut r = Raster::new(w, h);
    for p in r.data.chunks_exact_mut(4) {
        p.copy_from_slice(&[90, 140, 200, 255]);
    }
    r
}

fn base_config() -> Config {
    Config {
        shape: IconShape::Apple,
        subject: Subject::Original,
        tint: 0x3366cc,
        mono_style: MonoStyle::Tonal,
        plate_band: Band::Vivid,
        shortcut_shape: None,
        distinction: Distinction::None,
        mark_style: MarkStyle::Halo,
        mark_color: None,
        filter: FilterStyle::None,
        plate_color: None,
        plate_fallback: PlateFallback::Derived,
    }
}

#[test]
fn try_detect_background_survives_tiny_fully_opaque_sources() {
    // `try_uniform_rect_ring`'s inner inset (4) underflowed `width - 1 - inset` on a
    // board narrower than the inset (a 3×3 fully-opaque source crashed).
    for n in 1..=6 {
        let _ = try_detect_background(&opaque(n, n)); // must not panic
    }
}

#[test]
fn has_transparent_edges_handles_non_square_and_empty() {
    // The frozen form read `alpha_at(c, i, width-1)` — OOB when height < width.
    assert!(!has_transparent_edges(&opaque(8, 4)), "wide opaque → not floating");
    assert!(!has_transparent_edges(&opaque(4, 8)), "tall opaque → not floating");
    assert!(has_transparent_edges(&Raster::new(8, 4)), "wide empty → floating");
    assert!(has_transparent_edges(&Raster::new(4, 8)), "tall empty → floating");
    assert!(!has_transparent_edges(&Raster::new(0, 0)), "0×0 has no border → not floating");
}

#[test]
fn sticker_finish_on_a_tiny_tile_is_a_cleared_no_op() {
    // size ≤ 11 leaves no room after the die-cut margins; `size - 2*inset` underflowed.
    let mut tile = opaque(8, 8);
    apply_filter(&mut tile, 8, FilterStyle::Sticker, Subject::Original, 0x3366cc);
    assert!(tile.data.chunks_exact(4).all(|p| p == [0, 0, 0, 0]), "tiny sticker clears the tile");
}

#[test]
fn show_original_of_a_zero_dimension_source_is_transparent() {
    let mut diag = ComposeDiagnostics::default();
    let out =
        render_tile(&Raster::new(0, 0), &base_config(), false, true, 16, &RenderOpts::default(), &mut diag);
    assert_eq!((out.width, out.height), (16, 16));
    assert!(out.data.chunks_exact(4).all(|p| p == [0, 0, 0, 0]));
}

#[test]
fn styled_render_of_a_zero_dimension_source_takes_the_empty_lane() {
    // The styled path already guards 0×0 via the Empty lane (find/solid bounds); pinned
    // so the show-original fix above doesn't accidentally shift this behaviour.
    let mut diag = ComposeDiagnostics::default();
    let out =
        render_tile(&Raster::new(0, 0), &base_config(), false, false, 16, &RenderOpts::default(), &mut diag);
    assert_eq!(diag.lane, ComposeLane::Empty);
    assert!(out.data.chunks_exact(4).all(|p| p == [0, 0, 0, 0]));
}

#[test]
fn every_mark_renders_on_a_two_pixel_tile_without_underflow() {
    // A Ring inset (ring_stroke(2) == 2) drove `card_size = size - 2*pad` to underflow;
    // the (size - 1) / 2 pad clamp keeps the inscribed card at ≥ 1 px for every mark.
    for style in [
        MarkStyle::Glass,
        MarkStyle::Shadow,
        MarkStyle::Halo,
        MarkStyle::Satin,
        MarkStyle::Arc,
        MarkStyle::Fold,
        MarkStyle::Ring,
    ] {
        let mut config = base_config();
        config.distinction = Distinction::Mark;
        config.mark_style = style;
        let mut diag = ComposeDiagnostics::default();
        let out =
            render_tile(&opaque(2, 2), &config, true, false, 2, &RenderOpts::default(), &mut diag);
        assert_eq!((out.width, out.height), (2, 2), "{style:?} must render a 2×2 tile");
    }
}
