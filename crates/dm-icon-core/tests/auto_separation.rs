//! 自动分离 regression suite — the contrast-rescue stroke (`separation.rs`) at the
//! compose level. Three invariants:
//!   1. a melting rim + `auto_separation: true` draws the die-cut ring (bytes move,
//!      dark ink appears on the lane that previously rendered the subject invisible);
//!   2. a NON-melting rim renders byte-identical with the flag on and off — the flag
//!      alone never moves pixels, so frozen-parity surfaces stay certified;
//!   3. every rescued lane (Field user-plate, Field derived-plate, classic BareWhite)
//!      is covered, including the bimodal rim a mean-based detector would miss.

use dm_icon_core::compose::{render_tile, ComposeDiagnostics, ComposeFieldLane, ComposeLane, RenderOpts};
use dm_icon_core::config::{
    Band, Config, Distinction, FilterStyle, IconShape, MarkStyle, MonoStyle, PlateFallback, Subject,
};
use dm_icon_core::raster::Raster;

fn cfg() -> Config {
    Config {
        shape: IconShape::Circle,
        subject: Subject::Original,
        tint: 0x3366cc,
        mono_style: MonoStyle::Tonal,
        plate_band: Band::Vivid,
        shortcut_shape: None,
        distinction: Distinction::None,
        mark_style: MarkStyle::Glass,
        mark_color: None,
        filter: FilterStyle::None,
        plate_color: None,
        plate_fallback: PlateFallback::Derived,
        auto_separation: false,
    }
}

/// A plus-shaped glyph with mild per-pixel jitter on a transparent field.
/// Deliberately NOT a uniform rectangle/disc: a uniform solid block is (correctly)
/// classified by the background detectors as an icon's OWN BOARD and routes to the
/// plate lanes — these fixtures must stay in the BARE lanes, like a real logo.
fn glyph(rgb: (u8, u8, u8)) -> Raster {
    let mut r = Raster::new(64, 64);
    for y in 8..56 {
        for x in 8..56 {
            if !((24..40).contains(&x) || (24..40).contains(&y)) {
                continue;
            }
            let i4 = (y * 64 + x) * 4;
            let j = ((x + y) % 5) as u8;
            r.data[i4] = rgb.0.saturating_sub(j);
            r.data[i4 + 1] = rgb.1.saturating_sub(j);
            r.data[i4 + 2] = rgb.2.saturating_sub(j);
            r.data[i4 + 3] = 255;
        }
    }
    r
}

/// The same plus shape, left half light (melts on a light plate), right half
/// near-black (does not) — the bimodal rim a mean-based detector reads as safe.
fn bimodal_glyph() -> Raster {
    let mut r = Raster::new(64, 64);
    for y in 8..56 {
        for x in 8..56 {
            if !((24..40).contains(&x) || (24..40).contains(&y)) {
                continue;
            }
            let i4 = (y * 64 + x) * 4;
            let j = ((x + y) % 5) as u8;
            let v = if x < 32 { 230 - j } else { 20 + j };
            r.data[i4] = v;
            r.data[i4 + 1] = v;
            r.data[i4 + 2] = v;
            r.data[i4 + 3] = 255;
        }
    }
    r
}

fn render(artwork: &Raster, config: &Config) -> (Raster, ComposeDiagnostics) {
    let mut diag = ComposeDiagnostics::default();
    let tile = render_tile(artwork, config, false, false, 128, &RenderOpts::default(), &mut diag);
    (tile, diag)
}

/// Opaque pixels dark enough to be the rescue ink (the plates in these fixtures are
/// white/near-white; the soft dock shadow never blends anywhere near this dark).
fn dark_pixels(tile: &Raster) -> usize {
    tile.data
        .chunks_exact(4)
        .filter(|p| p[3] == 255 && (p[0] as u32 + p[1] as u32 + p[2] as u32) < 360)
        .count()
}

#[test]
fn a_white_glyph_on_a_user_white_plate_is_rescued_by_the_stroke() {
    let art = glyph((252, 252, 252));
    let base = Config { plate_color: Some(0xffffff), ..cfg() };
    let (off, diag_off) = render(&art, &base);
    let (on, diag_on) = render(&art, &Config { auto_separation: true, ..base });

    assert_eq!(diag_off.field_lane, Some(ComposeFieldLane::UserPlateBare));
    assert_eq!(diag_on.field_lane, Some(ComposeFieldLane::UserPlateBare));
    assert_ne!(off.data, on.data, "the melting rim must move pixels when the rescue is on");
    assert!(dark_pixels(&on) > dark_pixels(&off) + 50, "the rescue must draw a genuine ink ring");
}

#[test]
fn a_dark_glyph_on_a_user_white_plate_is_byte_untouched_by_the_flag() {
    // The flag alone must NEVER move pixels — only a genuinely melting rim does.
    let art = glyph((20, 20, 25));
    let base = Config { plate_color: Some(0xffffff), ..cfg() };
    let (off, _) = render(&art, &base);
    let (on, diag_on) = render(&art, &Config { auto_separation: true, ..base });

    assert_eq!(diag_on.field_lane, Some(ComposeFieldLane::UserPlateBare));
    assert_eq!(off.data, on.data, "a readable rim must not be 'rescued'");
}

#[test]
fn the_classic_bare_white_lane_rescues_a_white_glyph_it_previously_lost() {
    // plateFallback 'white' + bare artwork → BareWhite: white fill, NO shadow at
    // all — the lane where a white glyph used to vanish completely.
    let art = glyph((252, 252, 252));
    let base = Config { plate_fallback: PlateFallback::White, ..cfg() };
    let (off, diag_off) = render(&art, &base);
    let (on, diag_on) = render(&art, &Config { auto_separation: true, ..base });

    assert_eq!(diag_off.lane, ComposeLane::BareWhite);
    assert_eq!(diag_on.lane, ComposeLane::BareWhite);
    assert_ne!(off.data, on.data);
    assert!(dark_pixels(&off) == 0, "the frozen lane draws no dark pixel at all");
    assert!(dark_pixels(&on) > 50, "the rescue ring is the ONLY separation this lane has");
}

#[test]
fn a_bimodal_rim_on_a_derived_plate_is_rescued_without_moving_the_plate() {
    // Rim mean ~0.55 → the derivation picks the LIGHT plate line; the light half
    // of the rim melts against it. A mean-based detector reads this rim as safe —
    // the fraction detector must not.
    let art = bimodal_glyph();
    let base = cfg(); // plate_color None + fallback Derived → derived Field lane
    let (off, diag_off) = render(&art, &base);
    let (on, diag_on) = render(&art, &Config { auto_separation: true, ..base });

    assert_eq!(diag_off.field_lane, Some(ComposeFieldLane::DerivedBareShadow));
    assert_eq!(diag_on.field_lane, Some(ComposeFieldLane::DerivedBareShadow));
    assert_ne!(off.data, on.data, "the bimodal rim's light half must trigger the rescue");
}

/// A full-bleed opaque square with ANTI-DETECTION jitter: r and b ride
/// complementary phases (r−j, b−(30−j)), so per-pixel distance from the ring
/// mean reaches 2·|j−15| = 30 > both ring tolerances (18 canvas / 24 shape) on
/// enough pixels that neither background detector accepts it — while r+b stays
/// constant, keeping the perceived lightness (and thus the melt verdict) uniform.
/// No transparent edges, no own background, matches no shape → InscribeWhite.
fn noisy_full_square(rgb: (u8, u8, u8)) -> Raster {
    let mut r = Raster::new(64, 64);
    for y in 0..64 {
        for x in 0..64 {
            let i4 = (y * 64 + x) * 4;
            let j = ((x * 3 + y * 7) % 31) as u8;
            r.data[i4] = rgb.0.saturating_sub(j);
            r.data[i4 + 1] = rgb.1.saturating_sub(15);
            r.data[i4 + 2] = rgb.2.saturating_sub(30 - j);
            r.data[i4 + 3] = 255;
        }
    }
    r
}

#[test]
fn the_inscribe_white_lane_rescues_a_near_white_full_square() {
    let art = noisy_full_square((250, 250, 250));
    let base = Config { plate_fallback: PlateFallback::White, ..cfg() };
    let (off, diag_off) = render(&art, &base);
    let (on, diag_on) = render(&art, &Config { auto_separation: true, ..base });

    assert_eq!(diag_off.lane, ComposeLane::InscribeWhite);
    assert_eq!(diag_on.lane, ComposeLane::InscribeWhite);
    assert_ne!(off.data, on.data);
    assert!(dark_pixels(&on) > dark_pixels(&off) + 50);
}

#[test]
fn the_inscribe_white_lane_leaves_a_dark_full_square_byte_untouched() {
    let art = noisy_full_square((40, 44, 60));
    let base = Config { plate_fallback: PlateFallback::White, ..cfg() };
    let (off, diag_off) = render(&art, &base);
    let (on, _) = render(&art, &Config { auto_separation: true, ..base });

    assert_eq!(diag_off.lane, ComposeLane::InscribeWhite);
    assert_eq!(off.data, on.data, "no melt → the flag must not move a byte");
}

/// A NARROW plus-shape core wrapped in a 1px ring of `rgb`@`alpha` — models a real
/// artwork's anti-aliased fringe or a deliberate semi-transparent outline. Narrower
/// arms (12px) + a 1px ring keep the opaque coverage of the content bounds well
/// under the 0.62 shape-background gate, so the fixture stays a BARE profile (a
/// fatter ring pushed coverage over the gate and re-routed to the plate lanes).
fn glyph_with_soft_ring(core: (u8, u8, u8), ring: (u8, u8, u8), alpha: u8) -> Raster {
    let mut r = Raster::new(64, 64);
    let inside = |x: usize, y: usize| {
        (8..56).contains(&x)
            && (8..56).contains(&y)
            && ((26..38).contains(&x) || (26..38).contains(&y))
    };
    for y in 0..64usize {
        for x in 0..64usize {
            let i4 = (y * 64 + x) * 4;
            if inside(x, y) {
                let j = ((x + y) % 5) as u8;
                r.data[i4] = core.0.saturating_sub(j);
                r.data[i4 + 1] = core.1.saturating_sub(j);
                r.data[i4 + 2] = core.2.saturating_sub(j);
                r.data[i4 + 3] = 255;
                continue;
            }
            // Within 1px (Chebyshev) of the plus core → ring pixel.
            let near = (x.saturating_sub(1)..=(x + 1).min(63))
                .any(|nx| (y.saturating_sub(1)..=(y + 1).min(63)).any(|ny| inside(nx, ny)));
            if near {
                r.data[i4] = ring.0;
                r.data[i4 + 1] = ring.1;
                r.data[i4 + 2] = ring.2;
                r.data[i4 + 3] = alpha;
            }
        }
    }
    r
}

#[test]
fn a_dark_core_with_a_light_aa_fringe_is_not_falsely_rescued() {
    // The solid (α≥245) rim = the dark core edge, which reads fine on white — the
    // light α=160 fringe composites toward the plate and cannot melt what the core
    // carries. Verdict: no stroke, bytes untouched.
    let art = glyph_with_soft_ring((30, 30, 34), (250, 250, 250), 160);
    let base = Config { plate_color: Some(0xffffff), ..cfg() };
    let (off, _) = render(&art, &base);
    let (on, diag_on) = render(&art, &Config { auto_separation: true, ..base });

    assert_eq!(diag_on.field_lane, Some(ComposeFieldLane::UserPlateBare));
    assert_eq!(off.data, on.data);
}

#[test]
fn a_semi_transparent_design_outline_does_not_suppress_the_rescue() {
    // Accepted v1 behaviour (separation.rs module docs): a white core whose only
    // separation is a semi-transparent dark outline still triggers — the solid rim
    // is white-on-white — and the stroke thickens that outline (benign double
    // ring) rather than risking an invisible subject.
    let art = glyph_with_soft_ring((252, 252, 252), (40, 40, 44), 180);
    let base = Config { plate_color: Some(0xffffff), ..cfg() };
    let (off, _) = render(&art, &base);
    let (on, diag_on) = render(&art, &Config { auto_separation: true, ..base });

    assert_eq!(diag_on.field_lane, Some(ComposeFieldLane::UserPlateBare));
    assert_ne!(off.data, on.data);
}

#[test]
fn the_verdict_is_size_invariant_and_the_ring_survives_small_renders() {
    // Detection runs at SOURCE resolution: every output size must reach the same
    // verdict, and the proportional stroke must still land visible ink at 32px.
    let art = glyph((252, 252, 252));
    let base = Config { plate_color: Some(0xffffff), ..cfg() };
    for size in [32usize, 48, 128, 256] {
        let mut d_off = ComposeDiagnostics::default();
        let mut d_on = ComposeDiagnostics::default();
        let off = render_tile(&art, &base, false, false, size, &RenderOpts::default(), &mut d_off);
        let on = render_tile(
            &art,
            &Config { auto_separation: true, ..base },
            false,
            false,
            size,
            &RenderOpts::default(),
            &mut d_on,
        );
        assert_ne!(off.data, on.data, "melt verdict must hold at {size}px");
        assert!(dark_pixels(&on) > 0, "ink ring must survive at {size}px");
    }
}

#[test]
fn a_uniform_dark_glyph_on_its_derived_plate_never_triggers() {
    // The contrast derivation already opposes a unimodal rim — flag on must stay
    // byte-identical to flag off on the common path.
    let art = glyph((30, 60, 160));
    let base = cfg();
    let (off, _) = render(&art, &base);
    let (on, diag_on) = render(&art, &Config { auto_separation: true, ..base });

    assert_eq!(diag_on.field_lane, Some(ComposeFieldLane::DerivedBareShadow));
    assert_eq!(off.data, on.data);
}
