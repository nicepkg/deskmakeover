//! Cross-module algorithm properties + corpus-unreachable branches, exercised through
//! the crate's PUBLIC API (keeps the source files under the 500-line cap). Covers the
//! `Empty` compose lane (no source in the oracle corpus reaches it) and blur invariance.

use dm_icon_core::analysis::{find_content_bounds, solid_bounds, ContentBounds};
use dm_icon_core::compose::{
    render_tile, render_tile_cached, ComposeDiagnostics, ComposeFieldLane, ComposeLane, RenderOpts,
};
use dm_icon_core::config::{
    Band, Config, Distinction, FilterStyle, MarkStyle, MonoStyle, PlateFallback, Subject,
};
use dm_icon_core::js_math::{clamp_u8_int, js_round};
use dm_icon_core::profile::{IconProfile, IconProfileKind};
use dm_icon_core::raster::{backdrop_blur, box_blur, clip_to_mask, from_rgb_int, Raster};
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

#[test]
fn derived_plate_field_lane_via_the_render_session_profile_seam() {
    // The `DerivedPlate` field sub-lane is a faithful port of the frozen oracle but is
    // UNREACHABLE from `icon_profile(artwork)`: it needs kind==Bare && !transparent_edges,
    // yet the ≥90%-opaque border that !transparent_edges demands forces content coverage
    // ≥ 0.98, which the classifier routes to FullSquare BEFORE it ever detects a
    // background (profile.rs). No artwork — hence no RenderSession — reaches it in
    // production. We drive the ported branch through the same profile-override seam the
    // RenderSession uses, proving it composes a plate and records the lane.
    let mut artwork = Raster::new(64, 64);
    for y in 16..48 {
        for x in 16..48 {
            let i = (y * 64 + x) * 4;
            artwork.data[i..i + 4].copy_from_slice(&[200, 40, 40, 255]);
        }
    }
    let forced = IconProfile {
        kind: IconProfileKind::Bare,
        transparent_edges: false,
        background: None,
        background_lightness: None,
        subject_colour: None,
        subject_lightness: 0.4,
        subject_mask: None,
        subject_rim_colour: Some(from_rgb_int(0xcc3344)),
        subject_rim_lightness: 0.4,
    };
    let mut diag = ComposeDiagnostics::default();
    let mut mask_cache = dm_icon_core::mask_cache::MaskCache::new();
    let mut render_scratch = dm_icon_core::render_scratch::RenderScratch::new();
    let out = render_tile_cached(
        &artwork,
        &base_config(),
        false,
        false,
        64,
        &RenderOpts::default(),
        &mut diag,
        Some(&forced),
        &mut mask_cache,
        &mut render_scratch,
        None,
        None,
    );
    assert_eq!(diag.lane, ComposeLane::DerivedField);
    assert_eq!(diag.field_lane, Some(ComposeFieldLane::DerivedPlate));
    assert!(out.data.chunks_exact(4).any(|p| p[3] > 0), "the derived-plate lane must paint a tile");
}

#[test]
fn clip_to_mask_conserves_the_three_coverage_regimes() {
    // Property: cov ≤ 0 fully zeroes RGBA (compositeOver's alpha guard depends on it),
    // 0 < cov < 1 keeps RGB and scales alpha by round(a·cov), cov ≥ 1 leaves the pixel
    // byte-identical. (The raster.rs point test misses the cov ≥ 1 untouched branch.)
    let size = 24;
    let n = size * size;
    let mut r = Raster::new(size, size);
    for i in 0..n {
        let i4 = i * 4;
        r.data[i4] = (i % 251) as u8;
        r.data[i4 + 1] = ((i * 7) % 251) as u8;
        r.data[i4 + 2] = ((i * 13) % 251) as u8;
        r.data[i4 + 3] = 40 + (i % 200) as u8;
    }
    let before = r.data.clone();
    let cov = 0.375;
    let mask: Vec<f64> = (0..n).map(|i| match i % 3 { 0 => 0.0, 1 => cov, _ => 1.0 }).collect();
    clip_to_mask(&mut r, &mask);
    for i in 0..n {
        let i4 = i * 4;
        match i % 3 {
            0 => assert_eq!(&r.data[i4..i4 + 4], &[0, 0, 0, 0], "cov ≤ 0 must fully zero pixel {i}"),
            1 => {
                assert_eq!(&r.data[i4..i4 + 3], &before[i4..i4 + 3], "partial cov must keep RGB");
                let want = clamp_u8_int(js_round(before[i4 + 3] as f64 * cov));
                assert_eq!(r.data[i4 + 3], want, "partial cov must scale alpha by round(a·cov)");
            }
            _ => assert_eq!(&r.data[i4..i4 + 4], &before[i4..i4 + 4], "cov ≥ 1 must not touch the pixel"),
        }
    }
}

#[test]
fn content_bounds_and_solid_bounds_survive_a_zero_by_zero_raster() {
    // The genuinely-degenerate raster (no decode yields it) exercises the "no content
    // found" early-out in both scanners. NOTE: has_transparent_edges is NOT 0-width-safe
    // (`c.width - 1` underflows) — flagged to team-lead, deliberately not called here.
    let empty = Raster::new(0, 0);
    assert_eq!(find_content_bounds(&empty), ContentBounds { left: 0, top: 0, right: 0, bottom: 0 });
    assert_eq!(solid_bounds(&empty), None);
}

#[test]
#[should_panic(expected = "size must be positive")]
fn render_tile_rejects_a_zero_output_size() {
    // The output-size contract (compose.ts renderTile asserts size > 0); a 0-size tile is a
    // caller bug, pinned so the guard cannot be silently removed.
    let artwork = Raster::new(16, 16);
    let mut diag = ComposeDiagnostics::default();
    render_tile(&artwork, &base_config(), false, false, 0, &RenderOpts::default(), &mut diag);
}
