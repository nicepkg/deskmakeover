//! Cross-module algorithm properties + corpus-unreachable branches, exercised through
//! the crate's PUBLIC API (keeps the source files under the 500-line cap). Covers the
//! `Empty` compose lane (no source in the oracle corpus reaches it) and blur invariance.

use dm_icon_core::compose::{render_tile, ComposeDiagnostics, ComposeLane, RenderOpts};
use dm_icon_core::config::{
    Band, Config, Distinction, FilterStyle, MarkStyle, MonoStyle, PlateFallback, Subject,
};
use dm_icon_core::raster::{backdrop_blur, box_blur, Raster};
use dm_icon_core::shapes::IconShape;

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

fn render(artwork: &Raster, config: &Config) -> (Raster, ComposeLane) {
    let mut diag = ComposeDiagnostics::default();
    let out = render_tile(artwork, config, false, false, 256, &RenderOpts::default(), &mut diag);
    (out, diag.lane)
}

#[test]
fn a_single_sub_solid_speck_takes_the_empty_lane_and_blanks_the_tile() {
    // The ONLY way to trip the empty guard: one pixel with 24 < alpha < 128 — its content
    // bounds are exactly 1×1 and solid_bounds (alpha ≥ 128) is None. No oracle-corpus
    // source reaches this (M0b note); constructed here.
    let mut artwork = Raster::new(256, 256);
    let i = (128 * 256 + 128) * 4;
    artwork.data[i..i + 4].copy_from_slice(&[10, 20, 30, 100]);
    let (out, lane) = render(&artwork, &base_config());
    assert_eq!(lane, ComposeLane::Empty);
    assert!(out.data.chunks_exact(4).all(|p| p == [0, 0, 0, 0]), "empty lane must blank the tile");
}

#[test]
fn a_fully_transparent_source_does_not_trip_the_narrow_empty_guard() {
    // find_content_bounds returns the FULL canvas when nothing exceeds alpha 24, so the
    // guard's ≤1px content condition is NOT met — the empty lane is strictly the single
    // sub-solid speck above, not a blank source. (Pins the guard's exact width.)
    let (_out, lane) = render(&Raster::new(256, 256), &base_config());
    assert_ne!(lane, ComposeLane::Empty);
}

#[test]
fn a_shape_none_config_never_takes_empty_for_a_real_subject() {
    // Sanity anchor: a normal opaque subject does NOT hit the empty guard.
    let mut artwork = Raster::new(256, 256);
    for p in artwork.data.chunks_exact_mut(4) {
        p.copy_from_slice(&[120, 60, 200, 255]);
    }
    let (_out, lane) = render(&artwork, &base_config());
    assert_ne!(lane, ComposeLane::Empty);
}

#[test]
fn box_blur_of_a_constant_field_is_that_constant() {
    // Property: averaging a constant field returns the constant (every radius).
    let size = 32;
    let field = vec![0.7f64; size * size];
    for radius in [1, 2, 5, 12] {
        let out = box_blur(&field, size, radius);
        assert!(out.iter().all(|&v| (v - 0.7).abs() < 1e-9), "radius {radius} drifted");
    }
    // radius < 1 is an identity passthrough.
    assert_eq!(box_blur(&field, size, 0), field);
}

#[test]
fn backdrop_blur_of_a_uniform_raster_stays_that_colour() {
    let size = 32;
    let mut src = Raster::new(size, size);
    for p in src.data.chunks_exact_mut(4) {
        p.copy_from_slice(&[80, 140, 30, 255]);
    }
    let out = backdrop_blur(&src, 4);
    // Interior pixels (away from any edge handling) equal the source colour.
    for y in 8..24 {
        for x in 8..24 {
            let i = (y * size + x) * 4;
            assert_eq!(&out.data[i..i + 4], &[80, 140, 30, 255], "drift at {x},{y}");
        }
    }
    // radius < 1 is an identity clone.
    assert_eq!(backdrop_blur(&src, 0).data, src.data);
}
