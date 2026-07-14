//! Icon core (ADR-0019): pure pixel algorithms, planner, and RenderSession —
//! no platform, no I/O; the single pixel truth compiled to both wasm and native.
//!
//! Determinism contract (binding, ADR-0019 §Engineering discipline):
//! - transcendentals route through `libm` only (bit-identical wasm ↔ native);
//! - no `mul_add`/FMA, no SIMD, no order-dependent parallel reductions;
//! - TS `Float32Array`/`Float64Array` precision is mirrored field-by-field
//!   (`f32`/`f64`), including JS rounding semantics at byte boundaries
//!   (`js_math::js_round`, `js_math::clamp_byte`);
//! - every module is a 1:1 port of the FROZEN TS compositor
//!   (`src/icon-compositor/*`) — the external pixel
//!   contract is byte parity against that oracle.
//!
//! Ships the FULL compositor at HEAD (M5+ certified over the real corpus): raster primitives, the
//! shape catalog, sRGB/OKLab colour, analysis/segmentation, compose/marks/filters, `RenderSession`
//! caching, rayon batch render, and the content-addressed output cache. (Historically the M1 Spike-4
//! gate shipped only the slice modules; that milestone is long past.)

#![forbid(unsafe_code)]

pub mod analysis;
// Native rayon batch render across independent icons (M6 Phase 3). Excluded on wasm —
// the preview parallelizes with outer web workers, never a linked thread pool.
#[cfg(not(target_arch = "wasm32"))]
pub mod batch;
pub mod color;
// Guarded sRGB-encode byte LUT (M6 kernel-speed Phase 5) — `fast` build only; the
// scalar reference keeps the exact `libm::pow` in `color::srgb_encode`.
#[cfg(feature = "fast")]
pub mod srgb_lut;
pub mod compose;
pub mod config;
pub mod filters;
pub mod hue_spread;
pub mod js_math;
pub mod mask_cache;
pub mod marks;
pub mod mono;
// Content-addressed output cache (M6 Phase 4a). Excluded on wasm — the preview's exact
// session repeats are already free via the compositor styleLru.
#[cfg(not(target_arch = "wasm32"))]
pub mod output_cache;
pub mod profile;
pub mod raster;
pub mod render_scratch;
pub mod render_session;
pub mod sampling;
pub mod segment;
pub mod shape_facts;
pub mod source_facts;
pub mod shapes;
