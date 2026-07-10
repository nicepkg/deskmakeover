//! ICO container assembly — 1:1 port of the frozen C# `IcoWriter.cs`.
//!
//! Pure little-endian integer layout with NO floating point: an `ICONDIR` header, one
//! 16-byte `ICONDIRENTRY` per frame, then a 32-bit BGRA bottom-up DIB
//! (`BITMAPINFOHEADER` with `biHeight = height * 2`) plus an all-zero AND mask
//! (`stride = ((width + 31) / 32) * 4`) per frame. Because nothing here rounds, the
//! output is byte-identical to the C# oracle for the same frames — the container is the
//! differential gold standard (`crate` tests hold hand-computed C# goldens).

use dm_icon_core::raster::Raster;

const ICONDIR_LEN: usize = 6;
const ICONDIRENTRY_LEN: usize = 16;
const DIB_HEADER_LEN: usize = 40;

/// Assemble a multi-size `.ico` from straight-alpha RGBA frames (`IcoWriter.Write`).
/// Frames appear in the given order — the caller owns the ladder order (see
/// [`crate::ladder`]). Panics on an empty frame list, mirroring the C# `ArgumentException`.
pub fn write_ico(frames: &[Raster]) -> Vec<u8> {
    assert!(!frames.is_empty(), "at least one icon frame is required");

    let payloads: Vec<Vec<u8>> = frames.iter().map(dib_payload).collect();

    let mut out = Vec::with_capacity(
        ICONDIR_LEN
            + ICONDIRENTRY_LEN * frames.len()
            + payloads.iter().map(Vec::len).sum::<usize>(),
    );

    // ICONDIR: reserved=0, type=1 (icon), image count. Envelope: the directory count is
    // a u16, so at most 65,535 frames (the ladders ship 6).
    debug_assert!(frames.len() <= u16::MAX as usize, "ICO image count exceeds the u16 directory field");
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&(frames.len() as u16).to_le_bytes());

    // One ICONDIRENTRY per frame; image offsets accumulate past the directory.
    let mut image_offset = (ICONDIR_LEN + ICONDIRENTRY_LEN * frames.len()) as u32;
    for (frame, payload) in frames.iter().zip(&payloads) {
        out.push(dimension_byte(frame.width));
        out.push(dimension_byte(frame.height));
        out.push(0); // colorCount (0 = truecolor)
        out.push(0); // reserved
        out.extend_from_slice(&1u16.to_le_bytes()); // color planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        // Envelope: bytesInRes and the running offset are both u32 (a 256² frame is ~256 KB).
        debug_assert!(payload.len() <= u32::MAX as usize, "frame bytesInRes exceeds the u32 field");
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes()); // bytesInRes
        out.extend_from_slice(&image_offset.to_le_bytes());
        debug_assert!(
            (image_offset as u64) + (payload.len() as u64) <= u32::MAX as u64,
            "ICO image offset overflows the u32 field"
        );
        image_offset += payload.len() as u32;
    }

    for payload in &payloads {
        out.extend_from_slice(payload);
    }

    out
}

/// A dimension ≥256 stores as 0 in the directory entry (`IcoWriter` `>= 256 ? 0`).
fn dimension_byte(v: usize) -> u8 {
    if v >= 256 {
        0
    } else {
        v as u8
    }
}

/// One frame's DIB payload (`IcoWriter.CreateDibPayload`): `BITMAPINFOHEADER`, then
/// 32-bit BGRA scanlines BOTTOM-UP, then an all-zero AND mask.
fn dib_payload(frame: &Raster) -> Vec<u8> {
    let w = frame.width;
    let h = frame.height;
    let mask_stride = ((w + 31) / 32) * 4;
    let mut p = Vec::with_capacity(DIB_HEADER_LEN + w * h * 4 + mask_stride * h);

    // BITMAPINFOHEADER (40 bytes). biHeight is doubled: XOR image + AND mask.
    p.extend_from_slice(&(DIB_HEADER_LEN as i32).to_le_bytes());
    p.extend_from_slice(&(w as i32).to_le_bytes());
    p.extend_from_slice(&((h * 2) as i32).to_le_bytes());
    p.extend_from_slice(&1u16.to_le_bytes()); // planes
    p.extend_from_slice(&32u16.to_le_bytes()); // bit count
    p.extend_from_slice(&0i32.to_le_bytes()); // compression = BI_RGB
    p.extend_from_slice(&((w * h * 4) as i32).to_le_bytes()); // biSizeImage
    p.extend_from_slice(&0i32.to_le_bytes()); // x pixels/metre
    p.extend_from_slice(&0i32.to_le_bytes()); // y pixels/metre
    p.extend_from_slice(&0i32.to_le_bytes()); // colours used
    p.extend_from_slice(&0i32.to_le_bytes()); // colours important

    // BGRA scanlines, bottom-up (DIB origin is bottom-left).
    let d = &frame.data;
    for y in (0..h).rev() {
        for x in 0..w {
            let i = (y * w + x) * 4;
            p.push(d[i + 2]); // B
            p.push(d[i + 1]); // G
            p.push(d[i]); // R
            p.push(d[i + 3]); // A
        }
    }

    // AND mask — all zero; the 32-bit alpha channel is authoritative.
    p.resize(p.len() + mask_stride * h, 0);

    p
}

/// A parsed ICO directory entry paired with its frame's `BITMAPINFOHEADER` fields — for
/// structural verification, not pixel decoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IcoEntry {
    pub dir_width: u8,
    pub dir_height: u8,
    pub planes: u16,
    pub bit_count: u16,
    pub bytes_in_res: u32,
    pub image_offset: u32,
    pub dib_width: i32,
    pub dib_height: i32,
    pub dib_bit_count: u16,
    pub dib_size_image: i32,
}

/// Parse + validate the ICONDIR, entries, and each frame's `BITMAPINFOHEADER`. Confirms
/// the icon type, tightly packed monotonic offsets (each payload starts where the last
/// ended, first right after the directory), and that every `bytesInRes` equals the exact
/// DIB size for its dimensions — enough to assert a structurally valid multi-size ICO
/// without decoding pixels. Used by the codec tests and the `xtask m5-ico` corpus gate.
pub fn parse(bytes: &[u8]) -> Result<Vec<IcoEntry>, String> {
    if bytes.len() < ICONDIR_LEN {
        return Err("truncated ICONDIR".into());
    }
    let reserved = u16::from_le_bytes([bytes[0], bytes[1]]);
    let kind = u16::from_le_bytes([bytes[2], bytes[3]]);
    let count = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;
    if reserved != 0 {
        return Err(format!("reserved != 0: {reserved}"));
    }
    if kind != 1 {
        return Err(format!("type != 1 (icon): {kind}"));
    }
    if count == 0 {
        return Err("zero images".into());
    }

    let dir_end = ICONDIR_LEN + ICONDIRENTRY_LEN * count;
    if bytes.len() < dir_end {
        return Err("truncated directory".into());
    }

    let mut entries = Vec::with_capacity(count);
    let mut expected_offset = dir_end;
    for i in 0..count {
        let e = ICONDIR_LEN + i * ICONDIRENTRY_LEN;
        let bytes_in_res = u32::from_le_bytes([bytes[e + 8], bytes[e + 9], bytes[e + 10], bytes[e + 11]]);
        let image_offset =
            u32::from_le_bytes([bytes[e + 12], bytes[e + 13], bytes[e + 14], bytes[e + 15]]);
        if image_offset as usize != expected_offset {
            return Err(format!(
                "frame {i}: offset {image_offset} != packed {expected_offset}"
            ));
        }
        let start = image_offset as usize;
        let end = start
            .checked_add(bytes_in_res as usize)
            .ok_or_else(|| format!("frame {i}: offset overflow"))?;
        if end > bytes.len() {
            return Err(format!("frame {i}: payload [{start},{end}) escapes buffer"));
        }
        if (bytes_in_res as usize) < DIB_HEADER_LEN {
            return Err(format!("frame {i}: payload smaller than DIB header"));
        }

        let h = start;
        let dib_width = i32::from_le_bytes([bytes[h + 4], bytes[h + 5], bytes[h + 6], bytes[h + 7]]);
        let dib_height = i32::from_le_bytes([bytes[h + 8], bytes[h + 9], bytes[h + 10], bytes[h + 11]]);
        let dib_bit_count = u16::from_le_bytes([bytes[h + 14], bytes[h + 15]]);
        let dib_size_image =
            i32::from_le_bytes([bytes[h + 20], bytes[h + 21], bytes[h + 22], bytes[h + 23]]);

        if dib_width <= 0 || dib_height <= 0 || dib_height % 2 != 0 {
            return Err(format!("frame {i}: bad DIB dims {dib_width}x{dib_height}"));
        }
        let w = dib_width as usize;
        let hh = (dib_height / 2) as usize;
        let mask_stride = ((w + 31) / 32) * 4;
        // Checked so a crafted header cannot wrap the size arithmetic (wasm32 usize is
        // 32-bit; a malformed ICO must be rejected, never silently accepted after a wrap).
        let expected_res = w
            .checked_mul(hh)
            .and_then(|px| px.checked_mul(4))
            .and_then(|body| mask_stride.checked_mul(hh).and_then(|mask| body.checked_add(mask)))
            .and_then(|payload| payload.checked_add(DIB_HEADER_LEN))
            .ok_or_else(|| format!("frame {i}: DIB dimensions overflow the size field"))?;
        if bytes_in_res as usize != expected_res {
            return Err(format!(
                "frame {i}: bytesInRes {bytes_in_res} != DIB size {expected_res}"
            ));
        }

        entries.push(IcoEntry {
            dir_width: bytes[e],
            dir_height: bytes[e + 1],
            planes: u16::from_le_bytes([bytes[e + 4], bytes[e + 5]]),
            bit_count: u16::from_le_bytes([bytes[e + 6], bytes[e + 7]]),
            bytes_in_res,
            image_offset,
            dib_width,
            dib_height,
            dib_bit_count,
            dib_size_image,
        });
        expected_offset = end;
    }

    if expected_offset != bytes.len() {
        return Err(format!(
            "trailing bytes: packed end {expected_offset} != len {}",
            bytes.len()
        ));
    }

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(size: usize, r: u8, g: u8, b: u8, a: u8) -> Raster {
        let mut raster = Raster::new(size, size);
        for px in raster.data.chunks_exact_mut(4) {
            px[0] = r;
            px[1] = g;
            px[2] = b;
            px[3] = a;
        }
        raster
    }

    #[test]
    fn matches_hand_computed_csharp_bytes_for_a_2x2_red_frame() {
        // Independently derived from the frozen `IcoWriter.CreateDibPayload` for a 2x2
        // opaque red (R=255,G=0,B=0,A=255) frame — no `IcoWriter` call involved.
        #[rustfmt::skip]
        let expected: Vec<u8> = vec![
            // ICONDIR: reserved, type=1, count=1
            0x00, 0x00, 0x01, 0x00, 0x01, 0x00,
            // ICONDIRENTRY: w=2, h=2, colors=0, reserved=0, planes=1, bpp=32,
            //               bytesInRes=64, offset=22
            0x02, 0x02, 0x00, 0x00, 0x01, 0x00, 0x20, 0x00,
            0x40, 0x00, 0x00, 0x00, 0x16, 0x00, 0x00, 0x00,
            // BITMAPINFOHEADER: size=40, w=2, h=4 (2*2), planes=1, bpp=32,
            //                   compression=0, sizeImage=16, then four zero i32s
            0x28, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00,
            0x04, 0x00, 0x00, 0x00, 0x01, 0x00, 0x20, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            // BGRA pixels bottom-up: two rows, each 2px of B=0,G=0,R=255,A=255
            0x00, 0x00, 0xFF, 0xFF, 0x00, 0x00, 0xFF, 0xFF,
            0x00, 0x00, 0xFF, 0xFF, 0x00, 0x00, 0xFF, 0xFF,
            // AND mask: stride ((2+31)/32)*4 = 4, height 2 → 8 zero bytes
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        let got = write_ico(&[solid(2, 255, 0, 0, 255)]);
        assert_eq!(got, expected);
        assert_eq!(got.len(), 86);
    }

    #[test]
    fn replicates_the_frozen_csharp_header_test() {
        // Mirror of `IcoWriterTests.Writes_valid_ico_header_for_multiple_images`.
        let bytes = write_ico(&[solid(16, 255, 255, 255, 255), solid(32, 255, 255, 255, 255)]);
        assert_eq!(u16::from_le_bytes([bytes[0], bytes[1]]), 0);
        assert_eq!(u16::from_le_bytes([bytes[2], bytes[3]]), 1);
        assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), 2);
        assert!(bytes.len() > 6 + 32);
    }

    #[test]
    fn parse_accepts_and_describes_a_multi_size_ico() {
        let bytes = write_ico(&[solid(16, 10, 20, 30, 255), solid(32, 40, 50, 60, 128)]);
        let entries = parse(&bytes).expect("valid ICO");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].dir_width, 16);
        assert_eq!(entries[0].dib_width, 16);
        assert_eq!(entries[0].dib_height, 32); // 16 * 2
        assert_eq!(entries[0].bit_count, 32);
        assert_eq!(entries[0].planes, 1);
        assert_eq!(entries[1].dib_width, 32);
        assert_eq!(entries[1].dib_size_image, 32 * 32 * 4);
    }

    #[test]
    fn a_256_frame_stores_dimension_byte_zero_but_real_dib_width() {
        let bytes = write_ico(&[solid(256, 1, 2, 3, 255)]);
        let entries = parse(&bytes).expect("valid ICO");
        assert_eq!(entries[0].dir_width, 0); // ≥256 encodes as 0
        assert_eq!(entries[0].dir_height, 0);
        assert_eq!(entries[0].dib_width, 256);
        assert_eq!(entries[0].dib_height, 512);
    }

    #[test]
    fn parse_rejects_a_corrupt_type_field() {
        let mut bytes = write_ico(&[solid(16, 0, 0, 0, 0)]);
        bytes[2] = 2; // type 2 = CUR, not ICON
        assert!(parse(&bytes).is_err());
    }

    #[test]
    #[should_panic(expected = "at least one icon frame")]
    fn empty_frame_list_panics_like_csharp() {
        write_ico(&[]);
    }

    // ---- malformed-ICO rejection battery (every `parse` guard) ----

    #[test]
    fn parse_rejects_a_truncated_icondir() {
        assert!(parse(&[0, 0, 1]).is_err());
        assert!(parse(&[]).is_err());
    }

    #[test]
    fn parse_rejects_a_nonzero_reserved_field() {
        let mut b = write_ico(&[solid(16, 1, 2, 3, 255)]);
        b[0] = 1; // reserved must be 0
        assert!(parse(&b).unwrap_err().contains("reserved"));
    }

    #[test]
    fn parse_rejects_zero_image_count() {
        let mut b = write_ico(&[solid(16, 1, 2, 3, 255)]);
        b[4] = 0;
        b[5] = 0; // count = 0
        assert!(parse(&b).unwrap_err().contains("zero images"));
    }

    #[test]
    fn parse_rejects_a_truncated_directory() {
        // Header claims 2 images but carries no entries.
        let bytes = [0u8, 0, 1, 0, 2, 0];
        assert!(parse(&bytes).unwrap_err().contains("truncated directory"));
    }

    #[test]
    fn parse_rejects_a_non_packed_offset() {
        let mut b = write_ico(&[solid(16, 1, 2, 3, 255)]);
        // Entry imageOffset is at 6+12..6+16; bump it so it no longer equals dir_end.
        b[6 + 12] = b[6 + 12].wrapping_add(4);
        assert!(parse(&b).unwrap_err().contains("offset"));
    }

    #[test]
    fn parse_rejects_a_payload_escaping_the_buffer() {
        let mut b = write_ico(&[solid(16, 1, 2, 3, 255)]);
        // Inflate bytesInRes (entry +8..+12) far past the buffer.
        b[6 + 8] = 0xff;
        b[6 + 9] = 0xff;
        assert!(parse(&b).unwrap_err().contains("escapes buffer"));
    }

    #[test]
    fn parse_rejects_a_dib_size_mismatch() {
        let mut b = write_ico(&[solid(16, 1, 2, 3, 255), solid(32, 4, 5, 6, 255)]);
        // Frame 0 payload starts right after the 2-entry directory (6 + 32 = 38);
        // corrupt its DIB biWidth (payload +4) so the declared bytesInRes no longer
        // matches the size implied by the dimensions.
        b[38 + 4] = b[38 + 4].wrapping_add(3);
        assert!(parse(&b).is_err());
    }

    #[test]
    fn parse_rejects_trailing_bytes() {
        let mut b = write_ico(&[solid(16, 1, 2, 3, 255)]);
        b.push(0); // one byte past the last payload
        assert!(parse(&b).unwrap_err().contains("trailing"));
    }

    #[test]
    fn write_then_parse_round_trips_every_frame_structurally() {
        let frames: Vec<Raster> = [16usize, 20, 24, 32, 48, 256]
            .iter()
            .map(|&s| solid(s, (s % 256) as u8, 0, 0, 255))
            .collect();
        let entries = parse(&write_ico(&frames)).expect("valid ICO");
        assert_eq!(entries.len(), frames.len());
        for (e, f) in entries.iter().zip(&frames) {
            assert_eq!(e.dib_width as usize, f.width);
            assert_eq!(e.dib_height as usize, f.height * 2);
            assert_eq!(e.dib_size_image as usize, f.width * f.height * 4);
            assert_eq!(e.planes, 1);
            assert_eq!(e.bit_count, 32);
            assert_eq!(e.dir_width, dimension_byte(f.width));
        }
    }

    #[test]
    fn an_extreme_frame_count_is_valid_and_tightly_packed() {
        // 40 frames of the smallest size — offsets must stay monotonic + tight.
        let frames: Vec<Raster> = (0..40).map(|_| solid(16, 9, 9, 9, 255)).collect();
        let bytes = write_ico(&frames);
        let entries = parse(&bytes).expect("valid many-frame ICO");
        assert_eq!(entries.len(), 40);
        // Every payload identical (same frame) → identical bytesInRes; offsets step by it.
        let step = entries[0].bytes_in_res;
        for i in 1..entries.len() {
            assert_eq!(entries[i].image_offset, entries[i - 1].image_offset + step);
        }
    }

    #[test]
    fn a_non_square_frame_writes_distinct_width_height() {
        // The C# layout supports W≠H (width/height bytes written separately).
        let mut r = Raster::new(24, 16);
        r.data.chunks_exact_mut(4).for_each(|p| p.copy_from_slice(&[1, 2, 3, 255]));
        let entries = parse(&write_ico(&[r])).expect("valid non-square ICO");
        assert_eq!((entries[0].dir_width, entries[0].dir_height), (24, 16));
        assert_eq!((entries[0].dib_width, entries[0].dib_height), (24, 32));
    }
}
