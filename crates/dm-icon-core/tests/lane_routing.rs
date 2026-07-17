//! Compose lane routing pinned per lane. The 1487-cell cert already checks lane
//! assignment against the oracle, but every lane appearing in the corpus is a data
//! accident; these are the explicit constructive triggers (one minimal input per lane)
//! so a routing regression fails a fast unit test, not only the full certification.

use dm_icon_core::compose::{render_tile, ComposeDiagnostics, ComposeLane, RenderOpts};
use dm_icon_core::config::{
    Band, Config, Distinction, FilterStyle, MarkStyle, MonoStyle, PlateFallback, Subject,
};
use dm_icon_core::raster::Raster;
use dm_icon_core::shapes::IconShape;

fn base() -> Config {
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
        auto_separation: false,
    }
}

fn route(artwork: &Raster, config: &Config, show_original: bool) -> ComposeLane {
    let mut diag = ComposeDiagnostics::default();
    render_tile(artwork, config, false, show_original, 128, &RenderOpts::default(), &mut diag);
    diag.lane
}

/// A centred opaque blob on a transparent canvas (floating art: transparent edges,
/// no detectable background).
fn blob(size: usize) -> Raster {
    let mut r = Raster::new(size, size);
    let (lo, hi) = (size / 3, size * 2 / 3);
    for y in lo..hi {
        for x in lo..hi {
            let i = (y * size + x) * 4;
            r.data[i] = 60;
            r.data[i + 1] = 150;
            r.data[i + 2] = 210;
            r.data[i + 3] = 255;
        }
    }
    r
}

/// A floating plus-sign logo: transparent edges, and a bbox fill (~0.42) below the
/// plate gate, so no background is ever detected.
fn floating_logo(size: usize) -> Raster {
    let mut r = Raster::new(size, size);
    let (lo, hi) = (size / 4, size * 3 / 4);
    let c = size / 2;
    let mut set = |x: usize, y: usize| {
        let i = (y * size + x) * 4;
        r.data[i] = 200;
        r.data[i + 1] = 70;
        r.data[i + 2] = 90;
        r.data[i + 3] = 255;
    };
    for y in lo..hi {
        for x in (c - 2)..(c + 3) {
            set(x, y);
        }
    }
    for x in lo..hi {
        for y in (c - 2)..(c + 3) {
            set(x, y);
        }
    }
    r
}

/// A uniform opaque square — a detectable own-background board.
fn uniform_board(size: usize) -> Raster {
    let mut r = Raster::new(size, size);
    for p in r.data.chunks_exact_mut(4) {
        p.copy_from_slice(&[40, 90, 200, 255]);
    }
    r
}

/// A full-bleed checkerboard: opaque edges (not floating) with a non-uniform border
/// (no detectable background).
fn checkerboard(size: usize) -> Raster {
    let mut r = Raster::new(size, size);
    for y in 0..size {
        for x in 0..size {
            let i = (y * size + x) * 4;
            let on = ((x / 4) + (y / 4)) % 2 == 0;
            let v = if on { 230 } else { 30 };
            r.data[i] = v;
            r.data[i + 1] = v;
            r.data[i + 2] = v;
            r.data[i + 3] = 255;
        }
    }
    r
}

/// A solid disc filling the box — matches the Circle target shape.
fn disc(size: usize) -> Raster {
    let mut r = Raster::new(size, size);
    let c = (size as f64 - 1.0) / 2.0;
    let rad = size as f64 / 2.0;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f64 - c;
            let dy = y as f64 - c;
            if dx * dx + dy * dy <= rad * rad {
                let i = (y * size + x) * 4;
                r.data[i] = 200;
                r.data[i + 1] = 80;
                r.data[i + 2] = 60;
                r.data[i + 3] = 255;
            }
        }
    }
    r
}

#[test]
fn original_lane_is_the_show_original_path() {
    assert_eq!(route(&blob(64), &base(), true), ComposeLane::Original);
}

#[test]
fn empty_lane_for_a_single_sub_solid_speck() {
    let mut art = Raster::new(64, 64);
    let i = (32 * 64 + 32) * 4;
    art.data[i..i + 4].copy_from_slice(&[10, 20, 30, 100]);
    assert_eq!(route(&art, &base(), false), ComposeLane::Empty);
}

#[test]
fn derived_field_lane_for_original_subject_with_a_derived_plate() {
    assert_eq!(route(&blob(64), &base(), false), ComposeLane::DerivedField);
}

#[test]
fn passthrough_none_lane_for_the_none_shape() {
    let config = Config { shape: IconShape::None, ..base() };
    assert_eq!(route(&blob(64), &config, false), ComposeLane::PassthroughNone);
}

#[test]
fn layered_mono_lane_for_flat_mono() {
    let config = Config { subject: Subject::Mono, mono_style: MonoStyle::Flat, ..base() };
    assert_eq!(route(&blob(64), &config, false), ComposeLane::LayeredMono);
}

#[test]
fn passthrough_match_lane_when_the_art_is_the_target_shape() {
    // A disc under the Circle shape matches; a white plate fallback keeps us out of the
    // derived-field lane so the passthrough branch is reached.
    let config = Config { shape: IconShape::Circle, plate_fallback: PlateFallback::White, ..base() };
    assert_eq!(route(&disc(64), &config, false), ComposeLane::PassthroughMatch);
}

#[test]
fn plate_detect_lane_for_a_detectable_board() {
    let config = Config { plate_fallback: PlateFallback::White, ..base() };
    assert_eq!(route(&uniform_board(64), &config, false), ComposeLane::PlateDetect);
}

#[test]
fn bare_white_lane_for_a_floating_logo() {
    let config = Config { plate_fallback: PlateFallback::White, ..base() };
    assert_eq!(route(&floating_logo(64), &config, false), ComposeLane::BareWhite);
}

#[test]
fn inscribe_white_lane_for_a_full_bleed_inscribe_shape() {
    let config =
        Config { shape: IconShape::Diamond, plate_fallback: PlateFallback::White, ..base() };
    assert_eq!(route(&checkerboard(64), &config, false), ComposeLane::InscribeWhite);
}

#[test]
fn stretch_lane_for_a_full_bleed_non_inscribe_shape() {
    let config = Config { shape: IconShape::Lemon, plate_fallback: PlateFallback::White, ..base() };
    assert_eq!(route(&checkerboard(64), &config, false), ComposeLane::Stretch);
}
