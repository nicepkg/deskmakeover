//! Analysis-layer coverage through the public API: dominant-colour thresholds, the
//! hue-spread rotation cap, and the end-to-end plate split (segment_subject). These
//! pin the classifier gates the 256² corpus exercises only incidentally.

use dm_icon_core::analysis::dominant_color;
use dm_icon_core::color::to_ok_lab;
use dm_icon_core::hue_spread::{compute_hue_spread, SpreadEntry};
use dm_icon_core::raster::{hex_to_int, Raster};
use dm_icon_core::segment::{segment_subject, SegMode};

fn banded(size: usize, bands: &[([u8; 3], usize)]) -> Raster {
    // Vertical bands, each `(colour, row_count)`, fully opaque.
    let mut r = Raster::new(size, size);
    let mut y = 0;
    for (colour, rows) in bands {
        for _ in 0..*rows {
            if y >= size {
                break;
            }
            for x in 0..size {
                let i = (y * size + x) * 4;
                r.data[i] = colour[0];
                r.data[i + 1] = colour[1];
                r.data[i + 2] = colour[2];
                r.data[i + 3] = 255;
            }
            y += 1;
        }
    }
    r
}

// ---- dominant_color thresholds ----

#[test]
fn dominant_color_needs_chroma_above_the_min() {
    // An achromatic (gray) subject has no voter above DOMINANT_MIN_CHROMA → None.
    let gray = banded(32, &[([130, 130, 130], 32)]);
    assert!(dominant_color(&gray, None).is_none(), "a gray field has no dominant hue");
    // A saturated red subject clears the chroma gate → Some, red-dominant.
    let red = banded(32, &[([210, 40, 40], 32)]);
    let dom = dominant_color(&red, None).expect("saturated red has a dominant colour");
    assert!(dom.colour.r > dom.colour.g && dom.colour.r > dom.colour.b, "dominant is red-leaning");
}

#[test]
fn dominant_color_requires_a_theme_majority() {
    // 3 far-apart hues (red/green/blue) so no band merges across the 6-bucket span.
    // Peak (red) at ~41% is under THEME_MAJORITY → None.
    let minority = banded(64, &[([210, 40, 40], 26), ([40, 190, 60], 19), ([40, 70, 210], 19)]);
    assert!(dominant_color(&minority, None).is_none(), "a plurality-but-not-majority hue is rejected");
    // Push red to ~61% → its band is the majority → Some, red-dominant.
    let majority = banded(64, &[([210, 40, 40], 39), ([40, 190, 60], 13), ([40, 70, 210], 12)]);
    let dom = dominant_color(&majority, None).expect("a majority hue is accepted");
    assert!(dom.colour.r > dom.colour.b, "the accepted theme is the red majority");
}

// ---- hue-spread rotation cap ----

fn oklab_hue(hex: &str) -> f64 {
    let v = hex_to_int(hex);
    let (r, g, b) = (((v >> 16) & 0xff) as u8, ((v >> 8) & 0xff) as u8, (v & 0xff) as u8);
    let (_l, a, bb) = to_ok_lab(r, g, b);
    bb.atan2(a)
}

#[test]
fn hue_spread_never_rotates_a_plate_past_the_18_degree_cap() {
    // Six near-identical blues on DISTINCT artwork: relaxation wants 12° gaps (60° total)
    // but each plate may only rotate ±18° from its own hue, so the cap must bind. Every
    // output hue must stay within the cap of its seed.
    let seeds = ["#3366CC", "#3466CC", "#3266CE", "#3364CA", "#3568CC", "#3266C8"];
    let entries: Vec<SpreadEntry> = seeds
        .iter()
        .enumerate()
        .map(|(i, s)| SpreadEntry {
            id: format!("id{i}"),
            art_key: format!("art{i}"),
            seed: Some((*s).to_string()),
        })
        .collect();
    let out = compute_hue_spread(&entries);
    let cap = 18.0_f64.to_radians() + 2.0_f64.to_radians(); // + a small gamut/rounding margin
    for (i, seed) in seeds.iter().enumerate() {
        let hex = out.get(&format!("id{i}")).expect("every seeded id has a plate");
        let mut delta = (oklab_hue(hex) - oklab_hue(seed)).abs();
        if delta > std::f64::consts::PI {
            delta = std::f64::consts::TAU - delta; // shortest angular distance
        }
        assert!(delta <= cap, "id{i} rotated {}° > cap", delta.to_degrees());
    }
}

// ---- segment_subject plate detection (Otsu split + flat-plate) ----

/// A transparent-margin canvas with a solid square plate and a contrasting centred glyph.
fn plated_icon(size: usize) -> Raster {
    let mut r = Raster::new(size, size);
    let m = size / 8; // transparent margin → alpha-mode segmentation
    for y in m..size - m {
        for x in m..size - m {
            let i = (y * size + x) * 4;
            r.data[i] = 40;
            r.data[i + 1] = 90;
            r.data[i + 2] = 200;
            r.data[i + 3] = 255;
        }
    }
    let g0 = size * 3 / 8;
    let g1 = size * 5 / 8;
    for y in g0..g1 {
        for x in g0..g1 {
            let i = (y * size + x) * 4;
            r.data[i] = 245;
            r.data[i + 1] = 245;
            r.data[i + 2] = 245;
            r.data[i + 3] = 255;
        }
    }
    r
}

#[test]
fn segment_subject_splits_a_plate_and_reports_its_field() {
    let seg = segment_subject(&plated_icon(64));
    assert_eq!(seg.mode, SegMode::AlphaSplit, "a plate+glyph icon takes the alpha-split path");
    let field = seg.field.expect("the flat plate colour is detected");
    assert!(field.b > field.r && field.b > field.g, "the detected plate is the blue board");
}

#[test]
fn segment_subject_does_not_invent_a_plate_for_a_bare_glyph() {
    // A thin floating diagonal (low bbox fill) never passes the plate gate → no field.
    let size = 64;
    let mut r = Raster::new(size, size);
    for d in 8..size - 8 {
        let i = (d * size + d) * 4;
        r.data[i + 3] = 255;
        r.data[i] = 200;
    }
    let seg = segment_subject(&r);
    assert!(seg.field.is_none(), "a bare diagonal has no plate field");
}
