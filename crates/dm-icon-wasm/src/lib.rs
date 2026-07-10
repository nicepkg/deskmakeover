//! Icon wasm adapter (ADR-0019): wasm surface over dm-icon-core for the
//! in-window preview and manual bake — adapter only, no pixel logic of its own.
//!
//! Spike 4 exposes the slice via PLAIN `extern "C"` exports on
//! `wasm32-unknown-unknown` (no wasm-bindgen: the spike only moves raw RGBA
//! buffers, so the cheapest ABI that proves target parity is a linear-memory
//! pointer pair driven from Bun's `WebAssembly`; the full M6 adapter decides
//! wasm-bindgen vs raw exports for the worker-pool contract). The same crate
//! compiles natively too, so `cargo test` covers the adapter arithmetic.

mod abi;

use dm_icon_core::compose::{render_slice_tile, ComposeDiagnostics, RenderOpts};
use dm_icon_core::marks::set_native_arrow_raster;
use dm_icon_core::raster::Raster;
use dm_icon_core::render_session::RenderSession;

/// Allocate `len` bytes inside the module's linear memory; returns the offset.
/// The runner allocates once per buffer role and reuses across calls. Memory
/// is intentionally leaked — the spike instance is short-lived.
#[no_mangle]
pub extern "C" fn spike4_alloc(len: usize) -> *mut u8 {
    let mut buf = vec![0u8; len];
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// Render the Spike-4 slice: `src` is a straight-alpha RGBA source of
/// `src_w × src_h`; the `size × size × 4` result is written to `out`.
/// Returns 0 on success, 1 for a zero dimension, 2 if the source byte length
/// overflows `usize`.
///
/// The length is computed in `usize` with `checked_mul` — on `wasm32` `usize`
/// is 32-bit, so `src_w * src_h * 4` in `u32` would silently wrap. (M6 memo:
/// every `w * h * 4` in the future `render_tile` ABI must be checked the same
/// way.)
///
/// # Safety
/// `src` must point to `src_w * src_h * 4` readable bytes and `out` to
/// `size * size * 4` writable bytes inside linear memory (the runner
/// guarantees both via `spike4_alloc`).
#[no_mangle]
pub unsafe extern "C" fn spike4_render_slice(
    src: *const u8,
    src_w: u32,
    src_h: u32,
    size: u32,
    out: *mut u8,
) -> u32 {
    let (w, h, s) = (src_w as usize, src_h as usize, size as usize);
    if w == 0 || h == 0 || s == 0 {
        return 1;
    }
    let Some(src_len) = w.checked_mul(h).and_then(|n| n.checked_mul(4)) else {
        return 2;
    };
    let data = std::slice::from_raw_parts(src, src_len).to_vec();
    let artwork = Raster { width: w, height: h, data };
    let tile = render_slice_tile(&artwork, s);
    let out_slice = std::slice::from_raw_parts_mut(out, tile.data.len());
    out_slice.copy_from_slice(&tile.data);
    0
}

// ---- M6 worker-pool ABI -----------------------------------------------------
//
// The full preview adapter: one WASM instance per Web Worker owns one long-lived
// `RenderSession` (register-once sources + per-source profile cache; the session
// renders `&mut self`, so a shard needs no lock). All buffers cross as raw
// linear-memory pointers — no wasm-bindgen (keeps the structural wasm↔native
// byte-equality argument minimal). Every `w * h * 4` is `checked_mul`'d because
// `usize` is 32-bit on wasm32 and `u32` products would silently wrap.
//
// The instance leaks its scratch buffers by role (allocate once, reuse across
// renders) exactly as the spike path does; `dm_session_free` reclaims the
// session, not the pixel buffers.

/// Allocate `len` zeroed bytes in linear memory; returns the offset. The worker
/// allocates once per buffer role (source in, tile out, config, id) and reuses
/// across calls. Intentionally leaked for the instance lifetime.
#[no_mangle]
pub extern "C" fn dm_alloc(len: usize) -> *mut u8 {
    let mut buf = vec![0u8; len];
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// Install the genuine Win11 shortcut-arrow badge (the owner-extracted raster)
/// for this instance. Every worker sets it once at boot; until then
/// `draw_classic_arrow` uses the drawn fallback — which breaks byte-parity on
/// shortcut cells, so the worker MUST call this before rendering shortcuts. The
/// backing store is a process-global (P2-7), i.e. per WASM instance here.
/// Returns 0 ok; 1 null ptr; 3 zero/overflowing dims.
///
/// # Safety
/// `ptr[..w*h*4]` must be readable inside linear memory.
#[no_mangle]
pub unsafe extern "C" fn dm_set_native_arrow(ptr: *const u8, w: u32, h: u32) -> u32 {
    if ptr.is_null() {
        return 1;
    }
    let (w, h) = (w as usize, h as usize);
    if w == 0 || h == 0 {
        return 3;
    }
    let Some(len) = w.checked_mul(h).and_then(|n| n.checked_mul(4)) else {
        return 3;
    };
    let data = std::slice::from_raw_parts(ptr, len).to_vec();
    set_native_arrow_raster(Some(Raster { width: w, height: h, data }));
    0
}

/// Create a render session; returns an opaque handle (never null).
#[no_mangle]
pub extern "C" fn dm_session_new() -> *mut RenderSession {
    Box::into_raw(Box::new(RenderSession::new()))
}

/// Free a session created by `dm_session_new`. A null handle is a no-op.
///
/// # Safety
/// `s` must be a handle returned by `dm_session_new` and not already freed.
#[no_mangle]
pub unsafe extern "C" fn dm_session_free(s: *mut RenderSession) {
    if !s.is_null() {
        drop(Box::from_raw(s));
    }
}

/// Register (or replace) a `src_w × src_h` straight-alpha RGBA source under a
/// UTF-8 id and a caller-supplied content hash (the profile-cache key — the
/// caller owns its correctness, mirroring `RenderSession::register`).
/// Returns 0 ok; 1 null handle/ptr; 2 non-UTF-8 id; 3 zero/overflowing dims.
///
/// # Safety
/// `id_ptr[..id_len]` and `src_ptr[..src_w*src_h*4]` must be readable inside
/// linear memory (the worker guarantees both via `dm_alloc`).
#[no_mangle]
pub unsafe extern "C" fn dm_session_register(
    s: *mut RenderSession,
    id_ptr: *const u8,
    id_len: usize,
    source_hash: u64,
    src_ptr: *const u8,
    src_w: u32,
    src_h: u32,
) -> u32 {
    let Some(session) = s.as_mut() else {
        return 1;
    };
    if id_ptr.is_null() || src_ptr.is_null() {
        return 1;
    }
    let Ok(id) = std::str::from_utf8(std::slice::from_raw_parts(id_ptr, id_len)) else {
        return 2;
    };
    let (w, h) = (src_w as usize, src_h as usize);
    if w == 0 || h == 0 {
        return 3;
    }
    let Some(len) = w.checked_mul(h).and_then(|n| n.checked_mul(4)) else {
        return 3;
    };
    let data = std::slice::from_raw_parts(src_ptr, len).to_vec();
    session.register(id.to_owned(), source_hash, Raster { width: w, height: h, data });
    0
}

/// Decode a 24-byte packed config record (see `abi`) and set it as the current
/// look. Register-once: called per settings change, not per tile.
/// Returns 0 ok; 1 null handle/ptr; 2 short buffer or out-of-range enum tag.
///
/// # Safety
/// `cfg_ptr[..cfg_len]` must be readable inside linear memory.
#[no_mangle]
pub unsafe extern "C" fn dm_session_set_config(
    s: *mut RenderSession,
    cfg_ptr: *const u8,
    cfg_len: usize,
) -> u32 {
    let Some(session) = s.as_mut() else {
        return 1;
    };
    if cfg_ptr.is_null() {
        return 1;
    }
    match abi::parse_config(std::slice::from_raw_parts(cfg_ptr, cfg_len)) {
        Some(config) => {
            session.set_look(config);
            0
        }
        None => 2,
    }
}

/// Render a registered source under the current look into `out`
/// (`size × size × 4` bytes). `field_seed` is the hue-spread-adjusted seed as a
/// packed `0xRRGGBB` int, applied only when `has_field_seed != 0`.
/// Returns 0 ok; 1 null handle/ptr; 2 non-UTF-8 id; 3 zero/overflowing size;
/// 4 unknown id or no look set; 5 output length mismatch (never expected).
///
/// # Safety
/// `id_ptr[..id_len]` readable and `out[..size*size*4]` writable inside linear
/// memory.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn dm_session_render(
    s: *mut RenderSession,
    id_ptr: *const u8,
    id_len: usize,
    is_shortcut: u32,
    show_original: u32,
    size: u32,
    has_field_seed: u32,
    field_seed: u32,
    out: *mut u8,
) -> u32 {
    let Some(session) = s.as_mut() else {
        return 1;
    };
    if id_ptr.is_null() || out.is_null() {
        return 1;
    }
    let Ok(id) = std::str::from_utf8(std::slice::from_raw_parts(id_ptr, id_len)) else {
        return 2;
    };
    let sz = size as usize;
    if sz == 0 {
        return 3;
    }
    let Some(out_len) = sz.checked_mul(sz).and_then(|n| n.checked_mul(4)) else {
        return 3;
    };
    let opts = RenderOpts {
        field_seed: (has_field_seed != 0).then_some(field_seed),
    };
    let mut diag = ComposeDiagnostics::default();
    let Some(tile) = session.render(id, is_shortcut != 0, show_original != 0, sz, &opts, &mut diag)
    else {
        return 4;
    };
    if tile.data.len() != out_len {
        return 5;
    }
    std::slice::from_raw_parts_mut(out, out_len).copy_from_slice(&tile.data);
    0
}

/// Write a registered source's hue seed (subject rim colour) as 7 ASCII bytes
/// `#RRGGBB` into `out7`. Returns 1 if a seed exists (7 bytes written), 0 if the
/// source has no hue seed, 2 on a null/non-UTF-8/unexpected error. Feeds the
/// coordinator's hue-spread seed collection.
///
/// # Safety
/// `id_ptr[..id_len]` readable and `out7[..7]` writable inside linear memory.
#[no_mangle]
pub unsafe extern "C" fn dm_session_seed_of(
    s: *mut RenderSession,
    id_ptr: *const u8,
    id_len: usize,
    out7: *mut u8,
) -> u32 {
    let Some(session) = s.as_mut() else {
        return 2;
    };
    if id_ptr.is_null() || out7.is_null() {
        return 2;
    }
    let Ok(id) = std::str::from_utf8(std::slice::from_raw_parts(id_ptr, id_len)) else {
        return 2;
    };
    match session.seed_of(id) {
        Some(hex) if hex.len() == 7 => {
            std::slice::from_raw_parts_mut(out7, 7).copy_from_slice(hex.as_bytes());
            1
        }
        Some(_) => 2, // seed_of always yields "#RRGGBB"; a different length is a bug
        None => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_round_trips_native() {
        let src = vec![0u8; 16 * 16 * 4];
        let mut out = vec![0u8; 32 * 32 * 4];
        let code = unsafe { spike4_render_slice(src.as_ptr(), 16, 16, 32, out.as_mut_ptr()) };
        assert_eq!(code, 0);
        // Empty source → white plate clipped to the circle; centre is white.
        let c4 = (16 * 32 + 16) * 4;
        assert_eq!(&out[c4..c4 + 4], &[255, 255, 255, 255]);
    }

    fn gradient(w: usize, h: usize) -> Vec<u8> {
        let mut d = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 4;
                d[i] = (x * 255 / w.max(1)) as u8;
                d[i + 1] = (y * 255 / h.max(1)) as u8;
                d[i + 2] = 128;
                d[i + 3] = if (x + y) % 3 == 0 { 255 } else { 180 };
            }
        }
        d
    }

    /// The `extern "C"` ABI (pointer/len marshalling) must be a faithful pass-through
    /// to `render_slice_tile` — proven natively here; spike4 then proves native↔wasm
    /// byte-equality for the same export cross-runtime.
    #[test]
    fn abi_output_equals_core_over_varied_inputs() {
        for &(sw, sh, size) in &[(16usize, 16usize, 32usize), (64, 64, 256), (256, 256, 512), (200, 200, 48)] {
            let src = gradient(sw, sh);
            let mut out = vec![0u8; size * size * 4];
            let code = unsafe { spike4_render_slice(src.as_ptr(), sw as u32, sh as u32, size as u32, out.as_mut_ptr()) };
            assert_eq!(code, 0);
            let core = render_slice_tile(&Raster { width: sw, height: sh, data: src.clone() }, size);
            assert_eq!(out, core.data, "ABI output != core render at {sw}x{sh}->{size}");
        }
    }

    #[test]
    fn degenerate_dimensions_return_an_error_code_without_panicking() {
        // 0-dim would trip render_slice_tile's size assert; u32-space w*h*4 would wrap on
        // wasm32. Both return a non-zero code before any pointer read (src is a dummy).
        let src = vec![0u8; 4];
        let mut out = vec![0u8; 16 * 16 * 4];
        for &(w, h, s) in &[(0u32, 16u32, 16u32), (16, 0, 16), (16, 16, 0)] {
            let code = unsafe { spike4_render_slice(src.as_ptr(), w, h, s, out.as_mut_ptr()) };
            assert_eq!(code, 1, "degenerate {w}x{h}->{s} must return code 1");
        }
        // src byte length overflows usize (u32::MAX² × 4).
        let code = unsafe { spike4_render_slice(src.as_ptr(), u32::MAX, u32::MAX, 16, out.as_mut_ptr()) };
        assert_eq!(code, 2, "overflowing dimensions must return code 2");
    }

    #[test]
    fn alloc_returns_a_usable_zeroed_buffer() {
        let n = 4096;
        let ptr = spike4_alloc(n);
        assert!(!ptr.is_null());
        // Safety: `spike4_alloc` just gave us `n` writable, zeroed bytes.
        unsafe {
            let s = std::slice::from_raw_parts_mut(ptr, n);
            assert!(s.iter().all(|&b| b == 0), "alloc must zero the buffer");
            s[0] = 42;
            s[n - 1] = 7;
            assert_eq!((s[0], s[n - 1]), (42, 7));
        }
    }

    // ---- M6 session ABI ----

    fn solid(w: usize, h: usize, c: [u8; 4]) -> Vec<u8> {
        let mut d = vec![0u8; w * h * 4];
        for p in d.chunks_exact_mut(4) {
            p.copy_from_slice(&c);
        }
        d
    }

    /// `spectrum` as a packed 24-byte record: Circle / Original / Glass / None,
    /// tint 0xff6f5e, no optional colours, no shortcut shape.
    fn spectrum_bytes() -> [u8; abi::CONFIG_BYTES] {
        let mut b = [0u8; abi::CONFIG_BYTES];
        b[0] = 1; // shape Circle
        b[4] = 2; // distinction None
        b[8] = 0xFF; // shortcut_shape None
        b[12..16].copy_from_slice(&0x00ff_6f5e_u32.to_le_bytes());
        b
    }

    /// The extern "C" marshalling (register + config-decode + render) must be a
    /// faithful pass-through to `RenderSession` — proven natively here; the m6
    /// corpus harness then proves native↔wasm byte-equality over the 1487 cells.
    #[test]
    fn m6_abi_render_equals_render_session() {
        let src = solid(256, 256, [30, 120, 200, 255]);
        let cfg = spectrum_bytes();
        let s = dm_session_new();
        let mut out = vec![0u8; 256 * 256 * 4];
        unsafe {
            assert_eq!(dm_session_register(s, b"a".as_ptr(), 1, 0xABCD, src.as_ptr(), 256, 256), 0);
            assert_eq!(dm_session_set_config(s, cfg.as_ptr(), cfg.len()), 0);
            assert_eq!(
                dm_session_render(s, b"a".as_ptr(), 1, 0, 0, 256, 0, 0, out.as_mut_ptr()),
                0
            );
            dm_session_free(s);
        }
        // Direct RenderSession over the decoded config.
        let mut r = RenderSession::new();
        r.register("a", 0xABCD, Raster { width: 256, height: 256, data: src.clone() });
        r.set_look(abi::parse_config(&cfg).unwrap());
        let mut diag = ComposeDiagnostics::default();
        let direct = r.render("a", false, false, 256, &RenderOpts::default(), &mut diag).unwrap();
        assert_eq!(out, direct.data, "ABI render != RenderSession render");
    }

    #[test]
    fn m6_abi_error_codes_without_panicking() {
        let src = solid(256, 256, [10, 20, 30, 255]);
        let cfg = spectrum_bytes();
        let s = dm_session_new();
        let mut out = vec![0u8; 256 * 256 * 4];
        unsafe {
            // Null handle everywhere.
            let nil: *mut RenderSession = std::ptr::null_mut();
            assert_eq!(dm_session_register(nil, b"a".as_ptr(), 1, 0, src.as_ptr(), 256, 256), 1);
            assert_eq!(dm_session_set_config(nil, cfg.as_ptr(), cfg.len()), 1);
            assert_eq!(dm_session_render(nil, b"a".as_ptr(), 1, 0, 0, 256, 0, 0, out.as_mut_ptr()), 1);
            assert_eq!(dm_session_seed_of(nil, b"a".as_ptr(), 1, out.as_mut_ptr()), 2);

            // Zero dimension → 3; a short config → 2; render before a look is set → 4.
            assert_eq!(dm_session_register(s, b"a".as_ptr(), 1, 0, src.as_ptr(), 0, 256), 3);
            assert_eq!(dm_session_set_config(s, cfg.as_ptr(), abi::CONFIG_BYTES - 1), 2);
            assert_eq!(dm_session_register(s, b"a".as_ptr(), 1, 0, src.as_ptr(), 256, 256), 0);
            assert_eq!(
                dm_session_render(s, b"a".as_ptr(), 1, 0, 0, 256, 0, 0, out.as_mut_ptr()),
                4,
                "render before set_config must report no-look, not render"
            );
            // Unknown id after a look is set → 4.
            assert_eq!(dm_session_set_config(s, cfg.as_ptr(), cfg.len()), 0);
            assert_eq!(dm_session_render(s, b"z".as_ptr(), 1, 0, 0, 256, 0, 0, out.as_mut_ptr()), 4);
            dm_session_free(s);
        }
    }

    #[test]
    fn m6_seed_of_reports_presence() {
        let s = dm_session_new();
        let src = solid(256, 256, [200, 40, 40, 255]);
        let mut hex = [0u8; 7];
        unsafe {
            assert_eq!(dm_session_register(s, b"a".as_ptr(), 1, 7, src.as_ptr(), 256, 256), 0);
            let code = dm_session_seed_of(s, b"a".as_ptr(), 1, hex.as_mut_ptr());
            // A uniform red source yields a rim seed; if present it is "#RRGGBB".
            if code == 1 {
                assert_eq!(hex[0], b'#');
                assert!(hex[1..].iter().all(|b| b.is_ascii_hexdigit()));
            } else {
                assert_eq!(code, 0);
            }
            // Unknown id → no seed.
            assert_eq!(dm_session_seed_of(s, b"z".as_ptr(), 1, hex.as_mut_ptr()), 0);
            dm_session_free(s);
        }
    }
}
