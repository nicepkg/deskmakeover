//! Render-pipeline determinism scaffold (M6 kernel-speed plan Phase 0, task 4).
//!
//! The full 1487-cell cert proves byte parity vs the TS oracle for the SPECIFIC
//! corpus render order on a single thread. The fast-kernel phases add a session
//! mask cache (Phase 1), immutable source-fact caches (Phase 2), and native rayon
//! across icons (Phase 3) — each a new way to break determinism: a stale/aliased
//! cache entry, a copy-on-write miss that mutates a shared mask, an order- or
//! thread-dependent result. These tests pin the invariants those phases must keep,
//! so a regression fails a fast unit test, not only the full certification.
//!
//! They are GREEN today (no cache, single scalar path) and must STAY green under
//! `--features fast`. Self-contained: synthetic sources, no target/m5 dependency.

use dm_icon_core::batch::{render_icons_par_with_threads, IconJob};
use dm_icon_core::compose::{ComposeDiagnostics, RenderOpts};
use dm_icon_core::config::{
    Band, Config, Distinction, FilterStyle, MarkStyle, MonoStyle, PlateFallback, Subject,
};
use dm_icon_core::marks::set_native_arrow_raster;
use dm_icon_core::raster::Raster;
use dm_icon_core::render_session::RenderSession;
use dm_icon_core::shapes::IconShape;

// ── synthetic sources (mirror tests/lane_routing.rs shapes) ──────────────────

fn blob() -> Raster {
    let size = 256;
    let mut r = Raster::new(size, size);
    let (lo, hi) = (size / 3, size * 2 / 3);
    for y in lo..hi {
        for x in lo..hi {
            let i = (y * size + x) * 4;
            r.data[i..i + 4].copy_from_slice(&[60, 150, 210, 255]);
        }
    }
    r
}

fn board() -> Raster {
    let mut r = Raster::new(256, 256);
    for p in r.data.chunks_exact_mut(4) {
        p.copy_from_slice(&[40, 90, 200, 255]);
    }
    r
}

fn checkerboard() -> Raster {
    let size = 256;
    let mut r = Raster::new(size, size);
    for y in 0..size {
        for x in 0..size {
            let i = (y * size + x) * 4;
            let v = if ((x / 4) + (y / 4)) % 2 == 0 { 230 } else { 30 };
            r.data[i..i + 4].copy_from_slice(&[v, v, v, 255]);
        }
    }
    r
}

fn disc() -> Raster {
    let size = 256;
    let mut r = Raster::new(size, size);
    let c = (size as f64 - 1.0) / 2.0;
    let rad = size as f64 / 2.0;
    for y in 0..size {
        for x in 0..size {
            let (dx, dy) = (x as f64 - c, y as f64 - c);
            if dx * dx + dy * dy <= rad * rad {
                let i = (y * size + x) * 4;
                r.data[i..i + 4].copy_from_slice(&[200, 80, 60, 255]);
            }
        }
    }
    r
}

// ── render jobs: shapes × mark styles × sizes, incl. a shortcut + a Fold mark ─

struct Job {
    id: &'static str,
    source: Raster,
    config: Config,
    is_shortcut: bool,
    show_original: bool,
    size: usize,
    seed: Option<u32>,
}

fn cfg(shape: IconShape, mark: MarkStyle, distinction: Distinction) -> Config {
    Config {
        shape,
        subject: Subject::Original,
        tint: 0x3366cc,
        mono_style: MonoStyle::Tonal,
        plate_band: Band::Vivid,
        shortcut_shape: None,
        distinction,
        mark_style: mark,
        mark_color: None,
        filter: FilterStyle::None,
        plate_color: None,
        plate_fallback: PlateFallback::Derived,
    }
}

fn jobs() -> Vec<Job> {
    // Mark-bearing jobs set is_shortcut:true — marks (and Fold's in-place card carve)
    // resolve ONLY for shortcut cells (compose::render_tile_cached: `is_shortcut &&
    // distinction == Mark`). With is_shortcut:false the mark path was dead and the
    // COW test vacuous (Codex Phase-0 audit #4). apple-plain stays a no-mark sharer.
    vec![
        Job { id: "apple-halo", source: blob(), config: cfg(IconShape::Apple, MarkStyle::Halo, Distinction::Mark), is_shortcut: true, show_original: false, size: 256, seed: None },
        Job { id: "apple-fold", source: blob(), config: cfg(IconShape::Apple, MarkStyle::Fold, Distinction::Mark), is_shortcut: true, show_original: false, size: 256, seed: Some(0x00_ff_6f_5e) },
        Job { id: "apple-plain", source: board(), config: cfg(IconShape::Apple, MarkStyle::Glass, Distinction::None), is_shortcut: false, show_original: false, size: 256, seed: None },
        Job { id: "circle-glass", source: disc(), config: cfg(IconShape::Circle, MarkStyle::Glass, Distinction::Mark), is_shortcut: true, show_original: false, size: 256, seed: None },
        Job { id: "diamond-shadow", source: checkerboard(), config: { let mut c = cfg(IconShape::Diamond, MarkStyle::Shadow, Distinction::Mark); c.plate_fallback = PlateFallback::White; c }, is_shortcut: true, show_original: false, size: 128, seed: None },
        Job { id: "lemon-arc", source: checkerboard(), config: { let mut c = cfg(IconShape::Lemon, MarkStyle::Arc, Distinction::Mark); c.plate_fallback = PlateFallback::White; c }, is_shortcut: true, show_original: false, size: 96, seed: None },
        Job { id: "folder-ring", source: board(), config: cfg(IconShape::Folder, MarkStyle::Ring, Distinction::Mark), is_shortcut: true, show_original: false, size: 160, seed: None },
        Job { id: "apple-shortcut", source: blob(), config: cfg(IconShape::Apple, MarkStyle::Halo, Distinction::Mark), is_shortcut: true, show_original: false, size: 256, seed: None },
        Job { id: "circle-odd47", source: disc(), config: cfg(IconShape::Circle, MarkStyle::Glass, Distinction::Mark), is_shortcut: true, show_original: false, size: 47, seed: None },
    ]
}

/// A fixed synthetic shortcut badge so shortcut cells render deterministically. The
/// content is arbitrary but stable; every test installs the SAME bytes, so the
/// process-global `NATIVE_ARROW` RwLock stays value-stable under parallel tests.
fn install_arrow() {
    let mut a = Raster::new(100, 100);
    for (i, px) in a.data.chunks_exact_mut(4).enumerate() {
        if (i / 100 + i % 100) % 3 == 0 {
            px.copy_from_slice(&[255, 255, 255, 255]);
        }
    }
    set_native_arrow_raster(Some(a));
}

fn render_fresh(job: &Job) -> Vec<u8> {
    let mut s = RenderSession::new();
    s.register(job.id, 1, job.source.clone());
    s.set_look(job.config.clone());
    let mut d = ComposeDiagnostics::default();
    s.render(job.id, job.is_shortcut, job.show_original, job.size, &RenderOpts { field_seed: job.seed }, &mut d)
        .expect("render")
        .data
}

/// Reference outputs: each job rendered in its own fresh session (cold path).
fn references() -> (Vec<Job>, Vec<Vec<u8>>) {
    install_arrow();
    let jobs = jobs();
    let refs = jobs.iter().map(render_fresh).collect();
    (jobs, refs)
}

fn render_in_session(s: &mut RenderSession, job: &Job) -> Vec<u8> {
    s.set_look(job.config.clone());
    let mut d = ComposeDiagnostics::default();
    s.render(job.id, job.is_shortcut, job.show_original, job.size, &RenderOpts { field_seed: job.seed }, &mut d)
        .expect("render")
        .data
}

fn fresh_session_with_all(jobs: &[Job]) -> RenderSession {
    let mut s = RenderSession::new();
    for (i, job) in jobs.iter().enumerate() {
        s.register(job.id, i as u64 + 1, job.source.clone());
    }
    s
}

// ── determinism invariants ───────────────────────────────────────────────────

#[test]
fn hot_session_matches_cold_fresh_render() {
    let (jobs, refs) = references();
    let mut s = fresh_session_with_all(&jobs);
    for (i, job) in jobs.iter().enumerate() {
        assert_eq!(render_in_session(&mut s, job), refs[i], "hot session != fresh for {}", job.id);
    }
}

#[test]
fn repeated_render_is_idempotent() {
    let (jobs, refs) = references();
    let mut s = fresh_session_with_all(&jobs);
    for (i, job) in jobs.iter().enumerate() {
        let first = render_in_session(&mut s, job);
        let second = render_in_session(&mut s, job);
        assert_eq!(first, second, "repeat render differs for {}", job.id);
        assert_eq!(first, refs[i], "repeat render != fresh for {}", job.id);
    }
}

#[test]
fn randomized_render_order_is_order_independent() {
    let (jobs, refs) = references();
    // Deterministic xorshift permutation — render order must not change any output.
    let mut order: Vec<usize> = (0..jobs.len()).collect();
    let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
    for i in (1..order.len()).rev() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        order.swap(i, (x as usize) % (i + 1));
    }
    let mut s = fresh_session_with_all(&jobs);
    for &i in &order {
        assert_eq!(render_in_session(&mut s, &jobs[i]), refs[i], "shuffled order changed {}", jobs[i].id);
    }
}

#[test]
fn fold_mark_does_not_corrupt_shared_shape_masks() {
    // apple-fold and apple-plain share the (Apple, 256²) tile-alpha key; the Fold
    // carve mutates the CARD mask in place (marks/styles.rs carve_card) — in Phase 1
    // a copy-on-write of the cached entry — and a second Fold render at the same key
    // must not double-carve. Render Fold first, the sharer, then Fold again.
    let (jobs, refs) = references();
    let idx = |id: &str| jobs.iter().position(|j| j.id == id).unwrap();
    let (fold, plain) = (idx("apple-fold"), idx("apple-plain"));

    // Liveness: the Fold mark must actually carve+draw. If it renders identically to
    // the same cell with no mark, the mark path is dead (audit #4) and this test is
    // vacuous. This assertion is what would have caught the is_shortcut:false bug.
    let no_mark = Job {
        id: "apple-fold-nomark",
        source: jobs[fold].source.clone(),
        config: Config { distinction: Distinction::None, ..jobs[fold].config.clone() },
        is_shortcut: jobs[fold].is_shortcut,
        show_original: false,
        size: jobs[fold].size,
        seed: jobs[fold].seed,
    };
    assert_ne!(render_fresh(&jobs[fold]), render_fresh(&no_mark), "Fold mark did not carve — mark path is dead");

    let mut s = fresh_session_with_all(&jobs);
    assert_eq!(render_in_session(&mut s, &jobs[fold]), refs[fold], "apple-fold");
    assert_eq!(render_in_session(&mut s, &jobs[plain]), refs[plain], "apple-plain after fold carve");
    assert_eq!(render_in_session(&mut s, &jobs[fold]), refs[fold], "apple-fold second render (double-carve?)");
    assert_eq!(render_in_session(&mut s, &jobs[plain]), refs[plain], "apple-plain still intact");
}

#[test]
fn parallel_threads_match_single_thread() {
    let (jobs, refs) = references();
    // Wire the scaffold to the ACTUAL Phase-3 collector (`batch::render_icons_par`),
    // not a per-thread independent-session serial stand-in: rayon distributes these
    // independent icons across a bounded pool and `collect` returns them in INPUT
    // order. Asserting `out[i] == refs[i]` at 1/2/4/8 threads is the completion-order →
    // input-order ASSOCIATION gate (Codex Phase-0 audit #7): a collector that returned
    // completion-order pixels labeled in input order would go red here — the jobs are
    // deliberately different shapes/sizes, so a swap even changes the byte length.
    let icon_jobs: Vec<IconJob> = jobs
        .iter()
        .map(|j| IconJob {
            source: &j.source,
            config: &j.config,
            is_shortcut: j.is_shortcut,
            show_original: j.show_original,
            size: j.size,
            opts: RenderOpts { field_seed: j.seed },
        })
        .collect();
    for threads in [1usize, 2, 4, 8] {
        let out = render_icons_par_with_threads(&icon_jobs, threads);
        assert_eq!(out.len(), jobs.len(), "collector dropped/duplicated a job at {threads} threads");
        for (i, job) in jobs.iter().enumerate() {
            assert_eq!(out[i].data, refs[i], "threads={threads} misassociated or changed {}", job.id);
        }
    }
}
