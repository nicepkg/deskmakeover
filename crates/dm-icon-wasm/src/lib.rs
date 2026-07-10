//! Icon wasm adapter (ADR-0019): wasm surface over dm-icon-core for the
//! in-window preview and manual bake — adapter only, no pixel logic of its own.
//!
//! Spike 4 exposes the slice via PLAIN `extern "C"` exports on
//! `wasm32-unknown-unknown` (no wasm-bindgen: the spike only moves raw RGBA
//! buffers, so the cheapest ABI that proves target parity is a linear-memory
//! pointer pair driven from Bun's `WebAssembly`; the full M6 adapter decides
//! wasm-bindgen vs raw exports for the worker-pool contract). The same crate
//! compiles natively too, so `cargo test` covers the adapter arithmetic.

use dm_icon_core::raster::Raster;
use dm_icon_core::compose::render_slice_tile;

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
}
