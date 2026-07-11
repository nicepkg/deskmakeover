//! Native rayon batch render across independent icons (M6 kernel-speed Phase 3).
//!
//! Each icon is an INDEPENDENT output: a pure function of its own `(source, config,
//! size, flags, seed)` and the read-only process globals — `NATIVE_ARROW` (an
//! `RwLock` written only at boot), the `OnceLock` decode LUT, and the per-thread pure
//! `POLY_CACHE` / `RAMP_CACHE` memos. No render-path state is a shared mutable
//! accumulator, so rendering icons on different threads is byte-identical to rendering
//! them serially — the property Phase 3 exploits.
//!
//! Byte-safety rules honored here (the plan's Phase-3 red lines):
//!   • rayon parallelizes ONLY BETWEEN icons — never a within-icon floating-point
//!     reduction, which would reorder FP adds and break byte parity.
//!   • no mutex over a shared `RenderSession::render(&mut self)`; each task owns its
//!     render context — a per-worker `MaskCache` via `map_init`, a pure geometry memo
//!     that is byte-neutral (Phase-1 four-way certified: cache-on == cache-off).
//!   • `collect()` over an indexed parallel iterator returns results in INPUT order,
//!     so `out[i]` is always `render(jobs[i])` regardless of completion order.
//!
//! Downstream integration contract (for the src-tauri apply/background caller): the
//! byte-safety above relies on `NATIVE_ARROW` staying constant for the whole batch —
//! it is a boot-once RwLock. Do NOT call `set_native_arrow_raster` while a
//! `render_icons_par` is in flight; set the arrow once at startup, then render. A
//! concurrent write would let two icons in the same batch see different arrows.

use rayon::prelude::*;

use crate::compose::{render_tile_cached, ComposeDiagnostics, RenderOpts};
use crate::config::Config;
use crate::mask_cache::MaskCache;
use crate::raster::Raster;

/// One independent icon render request, borrowing its inputs from the caller's frozen
/// set (source + config + arrow are immutable during a batch).
pub struct IconJob<'a> {
    pub source: &'a Raster,
    pub config: &'a Config,
    pub is_shortcut: bool,
    pub show_original: bool,
    pub size: usize,
    pub opts: RenderOpts,
}

impl IconJob<'_> {
    /// Render this icon with a caller-owned scratch cache. Uncached analysis
    /// (`profile` / `source_facts` = `None`) — a batch is distinct sources, so there
    /// is nothing to reuse across icons; the per-worker `MaskCache` still collapses the
    /// shared shape-mask geometry.
    fn render(&self, mask_cache: &mut MaskCache) -> Raster {
        let mut diag = ComposeDiagnostics::default();
        render_tile_cached(
            self.source,
            self.config,
            self.is_shortcut,
            self.show_original,
            self.size,
            &self.opts,
            &mut diag,
            None,
            mask_cache,
            None,
        )
    }
}

/// Render independent icons across rayon's ambient pool, collected in INPUT order.
/// Byte-identical to a serial `render_tile` per job. `map_init` gives each worker its
/// own `MaskCache`, so Phase 1's shape-mask reuse survives without sharing mutable
/// state across threads; `collect` preserves the input index (a completion-order
/// collector would reorder the differently-sized tiles and fail the determinism
/// scaffold).
pub fn render_icons_par(jobs: &[IconJob]) -> Vec<Raster> {
    jobs.par_iter()
        .map_init(MaskCache::new, |cache, job| job.render(cache))
        .collect()
}

/// `render_icons_par` on a transient bounded pool — e.g. the background resident
/// renders with fewer threads than a foreground apply. Same byte-identical,
/// input-ordered result. Builds a one-shot pool; for steady-state throughput install a
/// pool once and call [`render_icons_par`] inside it.
pub fn render_icons_par_with_threads(jobs: &[IconJob], threads: usize) -> Vec<Raster> {
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads.max(1))
        .build()
        .expect("rayon pool")
        .install(|| render_icons_par(jobs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        Band, Config, Distinction, FilterStyle, MarkStyle, MonoStyle, PlateFallback, Subject,
    };
    use crate::shapes::IconShape;

    fn cfg(shape: IconShape, tint: u32) -> Config {
        Config {
            shape,
            subject: Subject::Original,
            tint,
            mono_style: MonoStyle::Tonal,
            plate_band: Band::Vivid,
            shortcut_shape: None,
            distinction: Distinction::None,
            mark_style: MarkStyle::Glass,
            mark_color: None,
            filter: FilterStyle::None,
            plate_color: None,
            plate_fallback: PlateFallback::Derived,
        }
    }

    /// A distinct opaque blob per seed so each job's output bytes differ — a
    /// misassociating collector would surface as a mismatch.
    fn blob(n: usize, r: u8, g: u8, b: u8) -> Raster {
        let mut raster = Raster::new(n, n);
        let (lo, hi) = (n / 4, n - n / 4);
        for y in lo..hi {
            for x in lo..hi {
                let i = (y * n + x) * 4;
                raster.data[i..i + 4].copy_from_slice(&[r, g, b, 255]);
            }
        }
        raster
    }

    fn serial(jobs: &[IconJob]) -> Vec<Vec<u8>> {
        jobs.iter().map(|j| j.render(&mut MaskCache::new()).data).collect()
    }

    /// The parallel collector must be byte-identical to a serial render, per job, and
    /// keep INPUT order at every thread count. Jobs vary shape/size/colour so an
    /// index/completion-order association bug shows up as a mismatch (a differently
    /// sized tile even fails on length).
    #[test]
    fn render_icons_par_matches_serial_and_keeps_order() {
        let sources: Vec<Raster> = vec![
            blob(256, 200, 60, 40),
            blob(128, 40, 200, 60),
            blob(96, 60, 40, 200),
            blob(160, 200, 200, 40),
            blob(256, 40, 200, 200),
        ];
        let cfgs = [
            cfg(IconShape::Circle, 0x3366cc),
            cfg(IconShape::Apple, 0xff6f5e),
            cfg(IconShape::Diamond, 0x33cc66),
            cfg(IconShape::Circle, 0xcc3366),
            cfg(IconShape::Apple, 0x6633cc),
        ];
        let sizes = [256, 128, 96, 160, 256];
        let jobs: Vec<IconJob> = (0..sources.len())
            .map(|i| IconJob {
                source: &sources[i],
                config: &cfgs[i],
                is_shortcut: false,
                show_original: false,
                size: sizes[i],
                opts: RenderOpts { field_seed: None },
            })
            .collect();

        let want = serial(&jobs);
        for threads in [1usize, 2, 4, 8] {
            let got = render_icons_par_with_threads(&jobs, threads);
            assert_eq!(got.len(), jobs.len());
            for (i, tile) in got.iter().enumerate() {
                assert_eq!(tile.data, want[i], "threads={threads}: job {i} != serial (misassociated or changed)");
            }
        }
    }

    /// Perf smoke — ignored by default (timing is machine/load-dependent). Run with
    /// `cargo test -p dm-icon-core --features fast --release native_rayon_throughput
    /// -- --ignored --nocapture` for the native multi-thread wall-clock delta.
    #[test]
    #[ignore = "perf smoke, run explicitly with --ignored --nocapture --release"]
    fn native_rayon_throughput() {
        use std::time::Instant;
        let sources: Vec<Raster> = (0..96).map(|i| blob(256, (i * 7) as u8, (i * 13) as u8, (i * 3) as u8)).collect();
        let shapes = [IconShape::Circle, IconShape::Apple, IconShape::Diamond];
        let cfgs: Vec<Config> = (0..96).map(|i| cfg(shapes[i % 3], 0x3366cc + (i as u32) * 17)).collect();
        let jobs: Vec<IconJob> = (0..96)
            .map(|i| IconJob {
                source: &sources[i],
                config: &cfgs[i],
                is_shortcut: false,
                show_original: false,
                size: 256,
                opts: RenderOpts { field_seed: None },
            })
            .collect();

        // Warm caches/thread pools, then take the best of 3 reps — contention only
        // ADDS wall-clock, so the minimum is the least-noisy signal.
        let base = serial(&jobs);
        let _ = serial(&jobs); // warmup
        let mut serial_ms = f64::INFINITY;
        for _ in 0..3 {
            let t = Instant::now();
            let _ = serial(&jobs);
            serial_ms = serial_ms.min(t.elapsed().as_secs_f64() * 1e3);
        }
        println!("serial: {serial_ms:.1} ms for {} icons @256px (best of 3)", jobs.len());
        for threads in [2usize, 4, 8] {
            let _ = render_icons_par_with_threads(&jobs, threads); // warmup
            let mut ms = f64::INFINITY;
            for _ in 0..3 {
                let t = Instant::now();
                let out = render_icons_par_with_threads(&jobs, threads);
                ms = ms.min(t.elapsed().as_secs_f64() * 1e3);
                for (i, tile) in out.iter().enumerate() {
                    assert_eq!(tile.data, base[i], "thread {threads} diverged at {i}");
                }
            }
            println!("  {threads} threads: {ms:.1} ms  ({:.2}× vs serial)", serial_ms / ms);
        }
    }
}
