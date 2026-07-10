//! M6 config ABI — the fixed 24-byte packed record the worker writes into linear
//! memory for `dm_session_set_config`. Mirrors `bridge/types.ts` `ConfigDto`
//! after the JS side has resolved the per-type ladder and pre-parsed colours to
//! packed `0xRRGGBB` ints (the same shape `RenderSession` / `Config` consumes).
//!
//! Enum tags follow each axis's TS union declaration order, so the JS encoder and
//! this decoder share ONE canonical numbering. A mismatch is not silent: a
//! mis-marshalled axis changes pixels and the 1487-cell byte-differential catches
//! it. Register-once semantics — `set_config` is called once per settings change,
//! never per tile — so a compact fixed layout (not a per-render parse) is correct.
//!
//! Layout (little-endian):
//! ```text
//!  0  shape           u8   IconShape tag 0..=11 (Apple..Folder)
//!  1  subject         u8   0 Original | 1 BlackWhite | 2 Mono
//!  2  mono_style      u8   0 Tonal | 1 Flat
//!  3  plate_band      u8   0 Vivid | 1 Quiet
//!  4  distinction     u8   0 Mark | 1 Keep | 2 None
//!  5  mark_style      u8   0 Glass 1 Shadow 2 Halo 3 Satin 4 Arc 5 Fold 6 Ring
//!  6  filter          u8   0 None 1 Gloss 2 Glass 3 Pixel 4 Sticker
//!  7  plate_fallback  u8   0 Derived ('derived') | 1 White ('white')
//!  8  shortcut_shape  u8   0xFF None, else IconShape tag
//!  9  has_mark_color  u8   0 | 1   (markColor null → 0)
//! 10  has_plate_color u8   0 | 1   (plateColor null → 0)
//! 11  (reserved)      u8   0
//! 12  tint            u32  hexToInt(config.tint)
//! 16  mark_color      u32  hexToInt(config.markColor), valid iff has_mark_color
//! 20  plate_color     u32  hexToInt(config.plateColor), valid iff has_plate_color
//! ```

use dm_icon_core::config::{
    Band, Config, Distinction, FilterStyle, IconShape, MarkStyle, MonoStyle, PlateFallback, Subject,
};

/// Byte length of the packed config record.
pub const CONFIG_BYTES: usize = 24;

/// IconShape ordinal, matching the `IconShape` TS union / `parse_shape` order.
fn shape_from_tag(tag: u8) -> Option<IconShape> {
    Some(match tag {
        0 => IconShape::Apple,
        1 => IconShape::Circle,
        2 => IconShape::Samsung,
        3 => IconShape::None,
        4 => IconShape::Bookmark,
        5 => IconShape::Lemon,
        6 => IconShape::Tile,
        7 => IconShape::Teardrop,
        8 => IconShape::Diamond,
        9 => IconShape::Flower,
        10 => IconShape::Pebble,
        11 => IconShape::Folder,
        _ => return None,
    })
}

fn subject_from_tag(tag: u8) -> Option<Subject> {
    Some(match tag {
        0 => Subject::Original,
        1 => Subject::BlackWhite,
        2 => Subject::Mono,
        _ => return None,
    })
}

fn mono_from_tag(tag: u8) -> Option<MonoStyle> {
    Some(match tag {
        0 => MonoStyle::Tonal,
        1 => MonoStyle::Flat,
        _ => return None,
    })
}

fn band_from_tag(tag: u8) -> Option<Band> {
    Some(match tag {
        0 => Band::Vivid,
        1 => Band::Quiet,
        _ => return None,
    })
}

fn distinction_from_tag(tag: u8) -> Option<Distinction> {
    Some(match tag {
        0 => Distinction::Mark,
        1 => Distinction::Keep,
        2 => Distinction::None,
        _ => return None,
    })
}

fn mark_from_tag(tag: u8) -> Option<MarkStyle> {
    Some(match tag {
        0 => MarkStyle::Glass,
        1 => MarkStyle::Shadow,
        2 => MarkStyle::Halo,
        3 => MarkStyle::Satin,
        4 => MarkStyle::Arc,
        5 => MarkStyle::Fold,
        6 => MarkStyle::Ring,
        _ => return None,
    })
}

fn filter_from_tag(tag: u8) -> Option<FilterStyle> {
    Some(match tag {
        0 => FilterStyle::None,
        1 => FilterStyle::Gloss,
        2 => FilterStyle::Glass,
        3 => FilterStyle::Pixel,
        4 => FilterStyle::Sticker,
        _ => return None,
    })
}

fn fallback_from_tag(tag: u8) -> Option<PlateFallback> {
    Some(match tag {
        0 => PlateFallback::Derived,
        1 => PlateFallback::White,
        _ => return None,
    })
}

/// Decode a packed config record. Returns `None` on a short buffer or any
/// out-of-range enum tag (the caller turns that into a non-zero ABI error code).
pub fn parse_config(b: &[u8]) -> Option<Config> {
    if b.len() < CONFIG_BYTES {
        return None;
    }
    let u32le = |o: usize| u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
    let shortcut_shape = match b[8] {
        0xFF => None,
        tag => Some(shape_from_tag(tag)?),
    };
    Some(Config {
        shape: shape_from_tag(b[0])?,
        subject: subject_from_tag(b[1])?,
        tint: u32le(12),
        mono_style: mono_from_tag(b[2])?,
        plate_band: band_from_tag(b[3])?,
        shortcut_shape,
        distinction: distinction_from_tag(b[4])?,
        mark_style: mark_from_tag(b[5])?,
        mark_color: (b[9] != 0).then(|| u32le(16)),
        filter: filter_from_tag(b[6])?,
        plate_color: (b[10] != 0).then(|| u32le(20)),
        plate_fallback: fallback_from_tag(b[7])?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A hand-built record for `spectrum` (Circle / Original / Glass / None,
    /// tint 0xff6f5e, no mark/plate colour, no shortcut shape).
    fn spectrum_bytes() -> [u8; CONFIG_BYTES] {
        let mut b = [0u8; CONFIG_BYTES];
        b[0] = 1; // shape Circle
        b[1] = 0; // subject Original
        b[2] = 0; // mono Tonal
        b[3] = 0; // band Vivid
        b[4] = 2; // distinction None
        b[5] = 0; // mark Glass
        b[6] = 0; // filter None
        b[7] = 0; // fallback Derived
        b[8] = 0xFF; // shortcut_shape None
        b[9] = 0; // has_mark_color
        b[10] = 0; // has_plate_color
        b[12..16].copy_from_slice(&0x00ff_6f5e_u32.to_le_bytes());
        b
    }

    #[test]
    fn decodes_a_full_record() {
        let c = parse_config(&spectrum_bytes()).expect("valid record");
        assert_eq!(c.shape, IconShape::Circle);
        assert_eq!(c.subject, Subject::Original);
        assert_eq!(c.mono_style, MonoStyle::Tonal);
        assert_eq!(c.plate_band, Band::Vivid);
        assert_eq!(c.distinction, Distinction::None);
        assert_eq!(c.mark_style, MarkStyle::Glass);
        assert_eq!(c.filter, FilterStyle::None);
        assert_eq!(c.plate_fallback, PlateFallback::Derived);
        assert_eq!(c.shortcut_shape, None);
        assert_eq!(c.mark_color, None);
        assert_eq!(c.plate_color, None);
        assert_eq!(c.tint, 0xff6f5e);
    }

    #[test]
    fn presence_flags_gate_the_optional_colours() {
        let mut b = spectrum_bytes();
        b[9] = 1; // has_mark_color
        b[10] = 1; // has_plate_color
        b[16..20].copy_from_slice(&0x0011_2233_u32.to_le_bytes());
        b[20..24].copy_from_slice(&0x0044_5566_u32.to_le_bytes());
        let c = parse_config(&b).unwrap();
        assert_eq!(c.mark_color, Some(0x112233));
        assert_eq!(c.plate_color, Some(0x445566));
        // A cleared flag ignores whatever bytes sit in the colour slot.
        b[9] = 0;
        assert_eq!(parse_config(&b).unwrap().mark_color, None);
    }

    #[test]
    fn shortcut_shape_sentinel_and_value() {
        let mut b = spectrum_bytes();
        assert_eq!(parse_config(&b).unwrap().shortcut_shape, None); // 0xFF
        b[8] = 11; // Folder
        assert_eq!(parse_config(&b).unwrap().shortcut_shape, Some(IconShape::Folder));
    }

    #[test]
    fn shape_tags_track_the_union_order() {
        // Guards the JS encoder ↔ Rust decoder shared numbering.
        let order = [
            IconShape::Apple,
            IconShape::Circle,
            IconShape::Samsung,
            IconShape::None,
            IconShape::Bookmark,
            IconShape::Lemon,
            IconShape::Tile,
            IconShape::Teardrop,
            IconShape::Diamond,
            IconShape::Flower,
            IconShape::Pebble,
            IconShape::Folder,
        ];
        for (i, want) in order.iter().enumerate() {
            assert_eq!(shape_from_tag(i as u8), Some(*want), "shape tag {i}");
        }
        assert_eq!(shape_from_tag(12), None);
    }

    #[test]
    fn rejects_short_buffer_and_bad_tags() {
        assert!(parse_config(&[0u8; CONFIG_BYTES - 1]).is_none());
        let mut b = spectrum_bytes();
        b[0] = 12; // out-of-range shape
        assert!(parse_config(&b).is_none());
        let mut b2 = spectrum_bytes();
        b2[6] = 5; // out-of-range filter
        assert!(parse_config(&b2).is_none());
    }
}
