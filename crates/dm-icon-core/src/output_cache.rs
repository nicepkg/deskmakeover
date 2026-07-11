//! Content-addressed output cache (M6 kernel-speed Phase 4a).
//!
//! A native/cross-session lever: apply and the background resident re-render the same
//! `(source, config, size, …)` on reopen / re-apply / a settings round-trip, and there
//! is no compositor `styleLru` on the native side. This caches the FINAL tile bytes by
//! a COMPLETE content digest, so a repeat is a clone instead of a recompute. (The wasm
//! preview already gets session repeats free via the compositor styleLru, so this
//! module is native-only.)
//!
//! Byte-safety (same discipline as Phases 1-2): the cache is gated on `fast`. The
//! default (scalar) build NEVER caches — it recomputes every tile, the determinism
//! reference. A cached tile is a prior `render_tile` output and `render_tile` is
//! deterministic, so a hit is byte-identical to a miss which is byte-identical to the
//! scalar recompute; the dedicated tests pin all three equal and the four-way cert
//! keeps `render_tile` itself 0-diff.
//!
//! The key is the load-bearing part: it must capture EVERY input that affects the
//! output, or a stale tile is served for a different render. It folds the source
//! pixel digest, the full resolved config, size, shortcut/original flags, the field
//! seed, the arrow digest, and the kernel/schema versions. blake3 makes it collision-
//! free (this also removes the Phase-2 `nextHash` collision worry for anything keyed
//! this way — different bytes ⇒ different key). blake3 hashes the KEY only, never the
//! pixels, so its internal SIMD is irrelevant to pixel parity.

use crate::compose::{render_tile, ComposeDiagnostics, RenderOpts};
use crate::config::Config;
use crate::raster::Raster;
// Folded into the content key (fast only — the scalar build never builds a key).
#[cfg(feature = "fast")]
use crate::{
    mask_cache::MASK_ALGO_VERSION, marks::native_arrow, render_session::ANALYSIS_SCHEMA_VERSION,
    source_facts::SOURCE_FACTS_SCHEMA_VERSION,
};

/// Bump on ANY change to the pixel output that the folded sub-schema versions
/// (mask/analysis/source-fact) do not already cover. A bump changes every key ⇒ full
/// cache invalidation.
pub const OUTPUT_CACHE_SCHEMA_VERSION: u32 = 1;

/// blake3 digest of a raster's pixels (dimensions folded in so a reshape can't alias).
/// Public because it is the collision-free content hash callers can key by.
pub fn source_digest(c: &Raster) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(&(c.width as u64).to_le_bytes());
    h.update(&(c.height as u64).to_le_bytes());
    h.update(&c.data);
    *h.finalize().as_bytes()
}

#[cfg(feature = "fast")]
fn arrow_digest() -> [u8; 32] {
    match native_arrow() {
        Some(a) => source_digest(&a),
        None => *blake3::hash(b"dm-icon:no-arrow").as_bytes(),
    }
}

/// The pre-hash key material — every output-affecting input, in a fixed order. Returned
/// as bytes (not a running hash) so tests can assert each axis is present and that a
/// per-axis change moves the material.
#[cfg(feature = "fast")]
fn content_key_material(
    source: &Raster,
    config: &Config,
    is_shortcut: bool,
    show_original: bool,
    size: usize,
    opts: &RenderOpts,
) -> Vec<u8> {
    let mut m = Vec::with_capacity(160);
    m.extend_from_slice(b"dm-icon:output-cache:v1"); // domain separation
    m.extend_from_slice(&source_digest(source));
    // Full resolved config — one field at a time. `as u32` on the fieldless config
    // enums is stable and self-guarding (a variant gaining data breaks the cast).
    m.extend_from_slice(&(config.shape as u32).to_le_bytes());
    m.extend_from_slice(&(config.subject as u32).to_le_bytes());
    m.extend_from_slice(&config.tint.to_le_bytes());
    m.extend_from_slice(&(config.mono_style as u32).to_le_bytes());
    m.extend_from_slice(&(config.plate_band as u32).to_le_bytes());
    push_opt_u32(&mut m, config.shortcut_shape.map(|s| s as u32));
    m.extend_from_slice(&(config.distinction as u32).to_le_bytes());
    m.extend_from_slice(&(config.mark_style as u32).to_le_bytes());
    push_opt_u32(&mut m, config.mark_color);
    m.extend_from_slice(&(config.filter as u32).to_le_bytes());
    push_opt_u32(&mut m, config.plate_color);
    m.extend_from_slice(&(config.plate_fallback as u32).to_le_bytes());
    // Render arguments beyond config.
    m.extend_from_slice(&(size as u64).to_le_bytes());
    m.push(is_shortcut as u8);
    m.push(show_original as u8);
    push_opt_u32(&mut m, opts.field_seed);
    m.extend_from_slice(&arrow_digest());
    // Kernel/schema versions — a bump to any of them changes every key.
    for v in [
        OUTPUT_CACHE_SCHEMA_VERSION,
        MASK_ALGO_VERSION,
        ANALYSIS_SCHEMA_VERSION,
        SOURCE_FACTS_SCHEMA_VERSION,
    ] {
        m.extend_from_slice(&v.to_le_bytes());
    }
    m
}

#[cfg(feature = "fast")]
fn push_opt_u32(m: &mut Vec<u8>, v: Option<u32>) {
    match v {
        None => m.push(0),
        Some(x) => {
            m.push(1);
            m.extend_from_slice(&x.to_le_bytes());
        }
    }
}

#[cfg(feature = "fast")]
fn content_key(
    source: &Raster,
    config: &Config,
    is_shortcut: bool,
    show_original: bool,
    size: usize,
    opts: &RenderOpts,
) -> [u8; 32] {
    *blake3::hash(&content_key_material(source, config, is_shortcut, show_original, size, opts)).as_bytes()
}

// ── the cache: real LRU under `fast`, a no-op passthrough under scalar ──────────────

#[cfg(feature = "fast")]
mod cache_impl {
    use super::Raster;
    use std::collections::HashMap;

    /// 64 MiB of cached tiles (~256 tiles at 512²) before byte-budget LRU eviction.
    pub const DEFAULT_CAP_BYTES: usize = 64 * 1024 * 1024;

    struct Entry {
        tile: Raster,
        last: u64,
    }

    /// Byte-budget LRU keyed by the 32-byte content digest. Eviction order is
    /// deterministic (lowest access tick), though eviction never affects OUTPUT bytes —
    /// an evicted key just recomputes to the same tile.
    pub struct OutputCache {
        entries: HashMap<[u8; 32], Entry>,
        tick: u64,
        bytes: usize,
        cap_bytes: usize,
    }

    impl OutputCache {
        pub fn new() -> Self {
            Self::with_cap(DEFAULT_CAP_BYTES)
        }
        pub fn with_cap(cap_bytes: usize) -> Self {
            Self { entries: HashMap::new(), tick: 0, bytes: 0, cap_bytes }
        }
        pub fn get(&mut self, key: &[u8; 32]) -> Option<Raster> {
            self.tick += 1;
            let t = self.tick;
            let e = self.entries.get_mut(key)?;
            e.last = t; // LRU bump
            Some(e.tile.clone())
        }
        pub fn insert(&mut self, key: [u8; 32], tile: Raster) {
            let sz = tile.data.len();
            if sz > self.cap_bytes {
                return; // a single tile larger than the whole budget is never cached
            }
            self.tick += 1;
            let last = self.tick;
            if let Some(old) = self.entries.insert(key, Entry { tile, last }) {
                self.bytes -= old.tile.data.len();
            }
            self.bytes += sz;
            self.evict();
        }
        fn evict(&mut self) {
            // Rare (only over budget); O(n) min-scan is fine for the tile counts in play.
            while self.bytes > self.cap_bytes {
                let Some(victim) = self.entries.iter().min_by_key(|(_, e)| e.last).map(|(k, _)| *k) else {
                    break;
                };
                if let Some(e) = self.entries.remove(&victim) {
                    self.bytes -= e.tile.data.len();
                }
            }
        }
        #[cfg(test)]
        pub fn len(&self) -> usize {
            self.entries.len()
        }
        #[cfg(test)]
        pub fn bytes(&self) -> usize {
            self.bytes
        }
        #[cfg(test)]
        pub fn contains(&self, key: &[u8; 32]) -> bool {
            self.entries.contains_key(key)
        }
    }
    impl Default for OutputCache {
        fn default() -> Self {
            Self::new()
        }
    }
}

#[cfg(not(feature = "fast"))]
mod cache_impl {
    /// Scalar reference: the cache holds nothing and every render recomputes.
    pub struct OutputCache;
    impl OutputCache {
        pub fn new() -> Self {
            Self
        }
        pub fn with_cap(_cap_bytes: usize) -> Self {
            Self
        }
    }
    impl Default for OutputCache {
        fn default() -> Self {
            Self::new()
        }
    }
}

pub use cache_impl::OutputCache;

/// Content-addressed render. On `fast`, an exact repeat (same source bytes + config +
/// size + flags + seed + arrow + kernel version) returns the cached tile; the default
/// (scalar) build ALWAYS recomputes — the determinism reference. A hit is byte-identical
/// to a miss (the cached tile is a prior deterministic `render_tile` output). `diag` is
/// populated only on a miss — it is diagnostic metadata, not part of the tile output.
pub fn render_tile_addressed(
    cache: &mut OutputCache,
    source: &Raster,
    config: &Config,
    is_shortcut: bool,
    show_original: bool,
    size: usize,
    opts: &RenderOpts,
    diag: &mut ComposeDiagnostics,
) -> Raster {
    #[cfg(feature = "fast")]
    {
        let key = content_key(source, config, is_shortcut, show_original, size, opts);
        if let Some(hit) = cache.get(&key) {
            return hit;
        }
        let tile = render_tile(source, config, is_shortcut, show_original, size, opts, diag);
        cache.insert(key, tile.clone());
        tile
    }
    #[cfg(not(feature = "fast"))]
    {
        let _ = cache;
        render_tile(source, config, is_shortcut, show_original, size, opts, diag)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        Band, Distinction, FilterStyle, MarkStyle, MonoStyle, PlateFallback, Subject,
    };
    use crate::shapes::IconShape;

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

    fn cfg() -> Config {
        Config {
            shape: IconShape::Circle,
            subject: Subject::Original,
            tint: 0x3366cc,
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

    fn free(source: &Raster, config: &Config, is_shortcut: bool, show_original: bool, size: usize, opts: &RenderOpts) -> Vec<u8> {
        let mut d = ComposeDiagnostics::default();
        render_tile(source, config, is_shortcut, show_original, size, opts, &mut d).data
    }

    // ── content hash (both kernels) ─────────────────────────────────────────────

    #[test]
    fn source_digest_stable_and_distinct() {
        let a = blob(64, 200, 60, 40);
        let a2 = blob(64, 200, 60, 40);
        let b = blob(64, 200, 60, 41); // one byte different
        assert_eq!(source_digest(&a), source_digest(&a2), "same pixels must digest equal");
        assert_ne!(source_digest(&a), source_digest(&b), "one-byte change must move the digest");
        // A reshape with the same bytes must not alias (dims folded in).
        let mut wide = Raster::new(128, 32);
        wide.data.copy_from_slice(&a.data);
        assert_ne!(source_digest(&a), source_digest(&wide), "reshape must move the digest");
    }

    #[test]
    fn distinct_sources_distinct_digests_no_collision() {
        let mut seen = std::collections::HashSet::new();
        for i in 0..256u16 {
            let d = source_digest(&blob(48, (i & 0xff) as u8, (i >> 1) as u8, (i >> 2) as u8));
            assert!(seen.insert(d), "digest collision at {i}");
        }
    }

    // ── cache correctness (fast only — scalar has no cache) ──────────────────────

    #[test]
    #[cfg(feature = "fast")]
    fn hit_equals_miss_equals_free_render() {
        let src = blob(128, 200, 60, 40);
        let c = cfg();
        let opts = RenderOpts { field_seed: None };
        let mut cache = OutputCache::new();
        let mut d = ComposeDiagnostics::default();
        let miss = render_tile_addressed(&mut cache, &src, &c, false, false, 256, &opts, &mut d);
        let hit = render_tile_addressed(&mut cache, &src, &c, false, false, 256, &opts, &mut d);
        assert_eq!(cache.len(), 1, "second render must be a hit, not a second entry");
        assert_eq!(miss.data, hit.data, "hit != miss");
        assert_eq!(hit.data, free(&src, &c, false, false, 256, &opts), "cached tile != free render_tile");
    }

    #[test]
    #[cfg(feature = "fast")]
    fn addressed_equals_free_over_job_set_with_repeats() {
        let sources = [blob(256, 200, 60, 40), blob(128, 40, 200, 60), blob(96, 60, 40, 200)];
        let cfgs = [
            cfg(),
            Config { shape: IconShape::Apple, mono_style: MonoStyle::Flat, subject: Subject::Mono, ..cfg() },
            Config { shape: IconShape::Diamond, tint: 0xcc3366, ..cfg() },
        ];
        let sizes = [256usize, 128, 96];
        let opts = RenderOpts { field_seed: None };
        let mut cache = OutputCache::new();
        // Two passes → the second is all hits; every tile must still equal a free render.
        for _pass in 0..2 {
            for i in 0..sources.len() {
                let mut d = ComposeDiagnostics::default();
                let got = render_tile_addressed(&mut cache, &sources[i], &cfgs[i], false, false, sizes[i], &opts, &mut d);
                assert_eq!(got.data, free(&sources[i], &cfgs[i], false, false, sizes[i], &opts), "job {i}: addressed != free");
            }
        }
        assert_eq!(cache.len(), sources.len(), "distinct jobs must be distinct entries; repeats must hit");
    }

    // ── key completeness: every output-affecting axis moves the key (fast only) ───

    #[test]
    #[cfg(feature = "fast")]
    fn key_completeness_each_axis_moves_the_key() {
        let src = blob(128, 200, 60, 40);
        let c = cfg();
        let opts = RenderOpts { field_seed: None };
        let base = content_key(&src, &c, false, false, 256, &opts);

        let mv = |k: [u8; 32], label: &str| assert_ne!(base, k, "axis did not move the key: {label}");

        mv(content_key(&blob(128, 200, 60, 41), &c, false, false, 256, &opts), "source");
        mv(content_key(&src, &Config { tint: 0x112233, ..c.clone() }, false, false, 256, &opts), "tint");
        mv(content_key(&src, &Config { shape: IconShape::Apple, ..c.clone() }, false, false, 256, &opts), "shape");
        mv(content_key(&src, &Config { subject: Subject::Mono, ..c.clone() }, false, false, 256, &opts), "subject");
        mv(content_key(&src, &Config { mono_style: MonoStyle::Flat, ..c.clone() }, false, false, 256, &opts), "mono_style");
        mv(content_key(&src, &Config { plate_band: Band::Quiet, ..c.clone() }, false, false, 256, &opts), "plate_band");
        mv(content_key(&src, &Config { shortcut_shape: Some(IconShape::Circle), ..c.clone() }, false, false, 256, &opts), "shortcut_shape");
        mv(content_key(&src, &Config { distinction: Distinction::Mark, ..c.clone() }, false, false, 256, &opts), "distinction");
        mv(content_key(&src, &Config { mark_style: MarkStyle::Halo, ..c.clone() }, false, false, 256, &opts), "mark_style");
        mv(content_key(&src, &Config { mark_color: Some(0x445566), ..c.clone() }, false, false, 256, &opts), "mark_color");
        mv(content_key(&src, &Config { filter: FilterStyle::Gloss, ..c.clone() }, false, false, 256, &opts), "filter");
        mv(content_key(&src, &Config { plate_color: Some(0x778899), ..c.clone() }, false, false, 256, &opts), "plate_color");
        mv(content_key(&src, &Config { plate_fallback: PlateFallback::White, ..c.clone() }, false, false, 256, &opts), "plate_fallback");
        mv(content_key(&src, &c, true, false, 256, &opts), "is_shortcut");
        mv(content_key(&src, &c, false, true, 256, &opts), "show_original");
        mv(content_key(&src, &c, false, false, 128, &opts), "size");
        mv(content_key(&src, &c, false, false, 256, &RenderOpts { field_seed: Some(7) }), "field_seed");
    }

    #[test]
    #[cfg(feature = "fast")]
    fn key_material_folds_kernel_versions() {
        // Schema-bump invalidation: the version consts are in the key material, so a
        // bump changes every key. Freeze them here — a change without a deliberate bump
        // goes red (and would silently keep serving stale tiles otherwise).
        let src = blob(64, 10, 20, 30);
        let opts = RenderOpts { field_seed: None };
        let m = content_key_material(&src, &cfg(), false, false, 256, &opts);
        for v in [OUTPUT_CACHE_SCHEMA_VERSION, MASK_ALGO_VERSION, ANALYSIS_SCHEMA_VERSION, SOURCE_FACTS_SCHEMA_VERSION] {
            assert!(
                m.windows(4).any(|w| w == v.to_le_bytes()),
                "kernel version {v} not folded into the content key"
            );
        }
    }

    // ── byte-budget LRU eviction (fast only) ─────────────────────────────────────

    #[test]
    #[cfg(feature = "fast")]
    fn byte_budget_evicts_lru() {
        // Cap at 2 tiles' worth; insert 3 distinct → the least-recently-used is evicted,
        // bytes stay within budget, and a freshly-got entry survives.
        let tile_bytes = 64 * 64 * 4;
        let mut cache = OutputCache::with_cap(tile_bytes * 2 + 1);
        let k = |n: u8| { let mut key = [0u8; 32]; key[0] = n; key };
        let tile = Raster::new(64, 64);
        cache.insert(k(1), tile.clone());
        cache.insert(k(2), tile.clone());
        assert!(cache.get(&k(1)).is_some(), "k1 present before eviction"); // bump k1 to MRU
        cache.insert(k(3), tile.clone()); // over budget → evict LRU (k2, not the just-touched k1)
        assert!(cache.bytes() <= tile_bytes * 2 + 1, "over byte budget after eviction");
        assert!(cache.contains(&k(1)), "recently-used k1 must survive");
        assert!(!cache.contains(&k(2)), "LRU k2 must be evicted");
        assert!(cache.contains(&k(3)), "just-inserted k3 present");
    }

    #[test]
    #[cfg(feature = "fast")]
    fn oversized_tile_is_not_cached() {
        let mut cache = OutputCache::with_cap(16);
        cache.insert([9u8; 32], Raster::new(64, 64));
        assert_eq!(cache.len(), 0, "a tile larger than the whole budget must not be stored");
    }

    // ── scalar reference: addressed == free, never caches (scalar build) ──────────

    /// Perf smoke — ignored by default. The cross-session/re-render win: a cold pass
    /// (all misses) then a warm pass (all hits ≈ a clone). Run with `cargo test -p
    /// dm-icon-core --features fast --release output_cache_hit_speedup -- --ignored
    /// --nocapture`.
    #[test]
    #[ignore = "perf smoke, run explicitly with --ignored --nocapture --release"]
    #[cfg(feature = "fast")]
    fn output_cache_hit_speedup() {
        use std::time::Instant;
        let sources: Vec<Raster> = (0..96).map(|i| blob(256, (i * 7) as u8, (i * 13) as u8, (i * 3) as u8)).collect();
        let shapes = [IconShape::Circle, IconShape::Apple, IconShape::Diamond];
        let cfgs: Vec<Config> = (0..96).map(|i| Config { shape: shapes[i % 3], tint: 0x3366cc + (i as u32) * 17, ..cfg() }).collect();
        let opts = RenderOpts { field_seed: None };
        let mut cache = OutputCache::new();
        let pass = |c: &mut OutputCache| -> f64 {
            let t = Instant::now();
            for i in 0..sources.len() {
                let mut d = ComposeDiagnostics::default();
                let _ = render_tile_addressed(c, &sources[i], &cfgs[i], false, false, 256, &opts, &mut d);
            }
            t.elapsed().as_secs_f64() * 1e3
        };
        let cold = pass(&mut cache); // all misses (first render / new session)
        let warm = pass(&mut cache); // all hits (reopen / re-apply / slider return)
        println!(
            "output cache: cold (all miss) {cold:.1} ms, warm (all hit) {warm:.1} ms for {} tiles @256px ({:.0}× faster on re-render)",
            sources.len(),
            cold / warm
        );
    }

    #[test]
    #[cfg(not(feature = "fast"))]
    fn scalar_addressed_equals_free_and_never_caches() {
        let src = blob(128, 200, 60, 40);
        let c = cfg();
        let opts = RenderOpts { field_seed: None };
        let mut cache = OutputCache::new();
        let mut d = ComposeDiagnostics::default();
        let a = render_tile_addressed(&mut cache, &src, &c, false, false, 256, &opts, &mut d);
        let b = render_tile_addressed(&mut cache, &src, &c, false, false, 256, &opts, &mut d);
        assert_eq!(a.data, b.data);
        assert_eq!(a.data, free(&src, &c, false, false, 256, &opts));
    }
}
