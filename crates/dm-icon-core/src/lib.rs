//! Icon core (ADR-0019): pure pixel algorithms, planner, and RenderSession —
//! no platform, no I/O; the single pixel truth compiled to both wasm and native.
//!
//! Determinism contract (binding, ADR-0019 §Engineering discipline):
//! - transcendentals route through `libm` only (bit-identical wasm ↔ native);
//! - no `mul_add`/FMA, no SIMD, no order-dependent parallel reductions;
//! - TS `Float32Array`/`Float64Array` precision is mirrored field-by-field
//!   (`f32`/`f64`), including JS rounding semantics at byte boundaries
//!   (`js_math::js_round`, `js_math::clamp_u8_round_half_even`);
//! - every module is a 1:1 port of the FROZEN TS compositor
//!   (`src/icon-compositor/*`) — the external pixel
//!   contract is byte parity against that oracle.
//!
//! Spike 4 (M1 gate) ships the slice modules only: raster primitives, Circle
//! shape, sRGB/OKLab colour (shadow tone), content bounds, sampling, and the
//! slice composition. The full compose/analysis/segment port lands at M5.

#![forbid(unsafe_code)]

pub mod analysis;
// Native rayon batch render across independent icons (M6 Phase 3). Excluded on wasm —
// the preview parallelizes with outer web workers, never a linked thread pool.
#[cfg(not(target_arch = "wasm32"))]
pub mod batch;
pub mod color;
pub mod compose;
pub mod config;
pub mod filters;
pub mod hue_spread;
pub mod js_math;
pub mod mask_cache;
pub mod marks;
pub mod mono;
pub mod profile;
pub mod raster;
pub mod render_session;
pub mod sampling;
pub mod segment;
pub mod source_facts;
pub mod shapes;
