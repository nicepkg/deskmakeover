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
/// Returns 0 on success (non-zero reserved for future error codes).
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
    let src_len = (src_w * src_h * 4) as usize;
    let data = std::slice::from_raw_parts(src, src_len).to_vec();
    let artwork = Raster { width: src_w as usize, height: src_h as usize, data };
    let tile = render_slice_tile(&artwork, size as usize);
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
}
