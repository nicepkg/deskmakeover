//! Session-owned geometry-mask memoization (M6 kernel-speed Phase 1).
//!
//! `shape_mask` (`raster.rs`) is a PURE function of `(shape, buffer_size, shape_size,
//! offset_x, offset_y)` but the compose path recomputes it on every render — a full
//! per-pixel corner classify + 16×16 boundary supersample, ~86% of render time
//! (Codex profile). The frozen TS oracle memoizes the identical masks; this ports
//! that. Because the masks are pure, caching is BYTE-NEUTRAL: it never changes a
//! single output bit, only avoids recompute. The (hardened) four-way cert enforces
//! that — the cached (`fast`) kernel must stay byte-identical to the recompute
//! (`scalar`) reference over the whole corpus AND the synthetic shape×mark sweep.
//!
//! Two builds behind the `fast` cargo feature:
//!   • `fast`   — a byte-capped cache keyed by the exact mask arguments.
//!   • default  — a passthrough that recomputes every call, the determinism
//!     reference the cert diffs `fast` against at every size and combination.

use std::sync::Arc;

use crate::shapes::IconShape;

/// Bumped if the mask-raster algorithm changes — folded into every key so a stale
/// entry from an older algorithm can never be served (belt for a future persisted
/// cache; within one process it is constant).
pub const MASK_ALGO_VERSION: u32 = 1;

/// Exact identity of a `shape_mask` result. The `f64` offsets are stored as raw
/// bits so the key is a total, hashable identity (no float `Eq`/`Hash` hazard) —
/// two calls collide in the cache ONLY when every argument is bit-identical.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct MaskKey {
    algo: u32,
    shape: IconShape,
    buffer_size: usize,
    shape_size: usize,
    offset_x_bits: u64,
    offset_y_bits: u64,
}

impl MaskKey {
    pub fn new(shape: IconShape, buffer_size: usize, shape_size: usize, offset_x: f64, offset_y: f64) -> Self {
        Self {
            algo: MASK_ALGO_VERSION,
            shape,
            buffer_size,
            shape_size,
            offset_x_bits: offset_x.to_bits(),
            offset_y_bits: offset_y.to_bits(),
        }
    }
}

/// Default per-session byte budget (16 MiB/worker). A 256² f64 mask + overhead is
/// 524,352 bytes, so this holds ~31 full-size masks — comfortably above the 256px
/// working set (geometry tile/card keys PLUS the Shadow stamp keys now routed through
/// the cache), so the common set never churns. 8 MiB held only 15 — one short of the
/// ~16-key set — so a 16th key evicted a live entry every render (recompute churn that
/// silently ate the Phase-1 win). Headroom is cheap; the cap only bounds the worst case.
pub const DEFAULT_CAP_BYTES: usize = 16 * 1024 * 1024;

/// Approximate fixed per-entry overhead (key + Arc header + map bucket) added to the
/// mask payload for the byte budget. Small and constant; the payload dominates. Only
/// the `fast` cache accounts bytes — the scalar passthrough stores nothing.
#[cfg(feature = "fast")]
const ENTRY_OVERHEAD: usize = 64;

#[cfg(feature = "fast")]
fn entry_bytes(mask: &[f64]) -> usize {
    mask.len() * std::mem::size_of::<f64>() + ENTRY_OVERHEAD
}

#[cfg(feature = "fast")]
mod imp {
    use super::*;
    use std::collections::{HashMap, VecDeque};

    /// Byte-capped mask cache with deterministic FIFO (insertion-order) eviction.
    /// FIFO is chosen over LRU because it is trivially deterministic — the same
    /// access sequence always evicts the same victim, so cache state is reproducible
    /// (a nondeterministic victim is exactly what the eviction cert forbids). Note
    /// the *policy* never affects output bytes: a miss recomputes the identical mask.
    pub struct MaskCache {
        map: HashMap<MaskKey, Arc<[f64]>>,
        order: VecDeque<MaskKey>,
        bytes: usize,
        cap_bytes: usize,
        // Observability for the eviction cert (not read by the render path).
        pub hits: u64,
        pub misses: u64,
        pub evictions: u64,
    }

    impl MaskCache {
        pub fn with_cap(cap_bytes: usize) -> Self {
            Self { map: HashMap::new(), order: VecDeque::new(), bytes: 0, cap_bytes, hits: 0, misses: 0, evictions: 0 }
        }

        pub fn new() -> Self {
            Self::with_cap(DEFAULT_CAP_BYTES)
        }

        /// Return the cached mask for `key`, or compute + store it. The returned
        /// `Arc<[f64]>` aliases the cached buffer — callers that MUTATE (Fold carve)
        /// must copy-on-write first (`.to_vec()`); read-only callers share it.
        pub fn get_or_compute(&mut self, key: MaskKey, compute: impl FnOnce() -> Vec<f64>) -> Arc<[f64]> {
            if let Some(mask) = self.map.get(&key) {
                self.hits += 1;
                return Arc::clone(mask);
            }
            self.misses += 1;
            let mask: Arc<[f64]> = Arc::from(compute());
            self.insert(key, Arc::clone(&mask));
            mask
        }

        fn insert(&mut self, key: MaskKey, mask: Arc<[f64]>) {
            let sz = entry_bytes(&mask);
            // A single mask larger than the whole budget is never cached (it would
            // evict everything then itself) — it just recomputes each time.
            if sz > self.cap_bytes {
                return;
            }
            self.map.insert(key, mask);
            self.order.push_back(key);
            self.bytes += sz;
            while self.bytes > self.cap_bytes {
                let Some(victim) = self.order.pop_front() else { break };
                if let Some(m) = self.map.remove(&victim) {
                    self.bytes -= entry_bytes(&m);
                    self.evictions += 1;
                }
            }
        }

        /// Live entry count (eviction cert).
        pub fn len(&self) -> usize {
            self.map.len()
        }

        pub fn is_empty(&self) -> bool {
            self.map.is_empty()
        }

        /// Current resident byte estimate (eviction cert).
        pub fn bytes(&self) -> usize {
            self.bytes
        }

        /// Whether `key` is currently resident. The eviction cert asserts the EXACT
        /// resident/evicted key set with this — aggregate counts alone can't catch a
        /// nondeterministic victim (a HashMap-order eviction can hit the same totals).
        pub fn contains(&self, key: &MaskKey) -> bool {
            self.map.contains_key(key)
        }
    }

    impl Default for MaskCache {
        fn default() -> Self {
            Self::new()
        }
    }
}

#[cfg(not(feature = "fast"))]
mod imp {
    use super::*;

    /// Passthrough reference: recompute every call, never store. This is the scalar
    /// determinism oracle — the cert diffs the `fast` cache against it.
    #[derive(Default)]
    pub struct MaskCache;

    impl MaskCache {
        pub fn new() -> Self {
            Self
        }

        pub fn with_cap(_cap_bytes: usize) -> Self {
            Self
        }

        pub fn get_or_compute(&mut self, _key: MaskKey, compute: impl FnOnce() -> Vec<f64>) -> Arc<[f64]> {
            Arc::from(compute())
        }
    }
}

pub use imp::MaskCache;

// The eviction cert (M6 Phase-0 audit #5/#10, folded into the cache's own commit as
// a hard gate). Only meaningful for the `fast` cache — the scalar passthrough has no
// state to evict. Proves: a HIT returns the bit-identical mask (no stale-key/reused-
// slot corruption); a forced eviction + revisit recomputes bit-identically; a
// randomized access order never returns a wrong mask; and FIFO eviction is
// deterministic (no nondeterministic victim). Bit comparisons use `f64::to_bits`.
#[cfg(all(test, feature = "fast"))]
mod eviction_cert {
    use super::*;
    use crate::raster::shape_mask;
    use crate::shapes::IconShape;

    fn bits(m: &[f64]) -> Vec<u64> {
        m.iter().map(|v| v.to_bits()).collect()
    }

    #[test]
    fn hit_returns_bit_identical_mask() {
        let mut c = MaskCache::new();
        let k = MaskKey::new(IconShape::Apple, 64, 64, 0.0, 0.0);
        let fresh = shape_mask(IconShape::Apple, 64, 64, 0.0, 0.0);
        let m1 = c.get_or_compute(k, || shape_mask(IconShape::Apple, 64, 64, 0.0, 0.0)); // miss
        let m2 = c.get_or_compute(k, || panic!("must be a HIT, not a recompute"));
        assert_eq!(c.misses, 1);
        assert_eq!(c.hits, 1);
        assert_eq!(bits(&m1), bits(&fresh));
        assert_eq!(bits(&m2), bits(&fresh), "hit returned a mask != fresh compute (stale key / reused slot)");
    }

    #[test]
    fn tile_and_card_share_one_entry_when_keys_match() {
        // The no-mark path keys the card mask at pad 0 = the tile-alpha key, so the
        // two collapse to a single shared entry (the frozen TS oracle shares too).
        let mut c = MaskCache::new();
        let k = MaskKey::new(IconShape::Apple, 64, 64, 0.0, 0.0);
        c.get_or_compute(k, || shape_mask(IconShape::Apple, 64, 64, 0.0, 0.0));
        c.get_or_compute(k, || shape_mask(IconShape::Apple, 64, 64, 0.0, 0.0));
        assert_eq!(c.len(), 1);
        assert_eq!(c.hits, 1);
    }

    #[test]
    fn forced_eviction_then_revisit_recomputes_bit_identical() {
        let a_fresh = shape_mask(IconShape::Circle, 32, 32, 0.0, 0.0);
        let cap = entry_bytes(&a_fresh) + 8; // room for exactly one 32² mask
        let mut c = MaskCache::with_cap(cap);
        let ka = MaskKey::new(IconShape::Circle, 32, 32, 0.0, 0.0);
        let kb = MaskKey::new(IconShape::Diamond, 32, 32, 0.0, 0.0);

        c.get_or_compute(ka, || shape_mask(IconShape::Circle, 32, 32, 0.0, 0.0)); // insert A
        c.get_or_compute(kb, || shape_mask(IconShape::Diamond, 32, 32, 0.0, 0.0)); // insert B → evict A
        assert_eq!(c.len(), 1, "cap of one mask must hold exactly one");
        assert!(c.evictions >= 1, "second insert over cap must evict");
        assert!(c.bytes() <= cap, "byte budget exceeded");

        // Revisiting A must be a fresh miss (it was evicted) and recompute bit-identically.
        let misses_before = c.misses;
        let a2 = c.get_or_compute(ka, || shape_mask(IconShape::Circle, 32, 32, 0.0, 0.0));
        assert_eq!(c.misses, misses_before + 1, "A should have been evicted (a real miss)");
        assert_eq!(bits(&a2), bits(&a_fresh), "recompute after eviction diverged");
    }

    #[test]
    fn randomized_order_never_returns_a_wrong_mask() {
        // A pool with distinct sizes/offsets; a small cap forces constant eviction.
        // Regardless of the hit/miss/evict interleaving, every returned mask must be
        // bit-identical to the fresh compute for its key.
        let pool: [(IconShape, usize, usize, f64, f64); 5] = [
            (IconShape::Apple, 64, 64, 0.0, 0.0),
            (IconShape::Circle, 64, 64, 0.0, 0.0),
            (IconShape::Diamond, 64, 50, 7.0, 7.0),
            (IconShape::Folder, 64, 64, 0.0, 0.0),
            (IconShape::Lemon, 48, 40, 4.0, 4.0),
        ];
        let fresh: Vec<Vec<u64>> = pool.iter().map(|&(s, b, ss, ox, oy)| bits(&shape_mask(s, b, ss, ox, oy))).collect();
        let cap = entry_bytes(&shape_mask(IconShape::Apple, 64, 64, 0.0, 0.0)) * 2;
        let mut c = MaskCache::with_cap(cap);
        let mut x: u64 = 0x1234_5678_9abc_def0;
        for _ in 0..300 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let i = (x as usize) % pool.len();
            let (s, b, ss, ox, oy) = pool[i];
            let m = c.get_or_compute(MaskKey::new(s, b, ss, ox, oy), || shape_mask(s, b, ss, ox, oy));
            assert_eq!(bits(&m), fresh[i], "key {i} returned a mask != fresh compute");
            assert!(c.bytes() <= cap, "byte budget exceeded mid-sequence");
        }
    }

    #[test]
    fn fifo_eviction_is_deterministic() {
        // Categorically pin the FIFO victim at EVERY eviction, not just the final
        // state. Final membership + counts are too weak: a deterministic-but-wrong
        // victim history (the classic A,D,C,A instead of A,C,D,A) lands the SAME final
        // set {Folder,Circle} and the SAME counts, yet is a wrong policy. So assert the
        // exact resident set after EACH insert (review P2-2 round-3, audit #10).
        let apple = MaskKey::new(IconShape::Apple, 64, 64, 0.0, 0.0);
        let circle = MaskKey::new(IconShape::Circle, 64, 64, 0.0, 0.0);
        let diamond = MaskKey::new(IconShape::Diamond, 64, 64, 0.0, 0.0);
        let folder = MaskKey::new(IconShape::Folder, 64, 64, 0.0, 0.0);

        // Each row: the shape inserted, and the exact resident set
        // [apple, circle, diamond, folder] required AFTER that insert. cap holds two
        // 64² masks, FIFO evicts the oldest, so the victim is fully pinned step by step.
        let steps: [(IconShape, [bool; 4]); 6] = [
            (IconShape::Apple, [true, false, false, false]),
            (IconShape::Circle, [true, true, false, false]),
            (IconShape::Diamond, [false, true, true, false]), // evict Apple (oldest)
            (IconShape::Apple, [true, false, true, false]),    // evict Circle; re-insert Apple
            (IconShape::Folder, [true, false, false, true]),   // evict Diamond
            (IconShape::Circle, [false, true, false, true]),   // evict Apple; re-insert Circle
        ];

        let cap = entry_bytes(&shape_mask(IconShape::Apple, 64, 64, 0.0, 0.0)) * 2 + 8;
        let run = || {
            let mut c = MaskCache::with_cap(cap);
            let mut trace = Vec::new();
            for &(s, _) in &steps {
                c.get_or_compute(MaskKey::new(s, 64, 64, 0.0, 0.0), || shape_mask(s, 64, 64, 0.0, 0.0));
                trace.push([c.contains(&apple), c.contains(&circle), c.contains(&diamond), c.contains(&folder)]);
            }
            (trace, c.len(), c.hits, c.misses, c.evictions)
        };

        let (trace, len, hits, misses, evictions) = run();
        for (i, (_, expected)) in steps.iter().enumerate() {
            assert_eq!(&trace[i], expected, "resident set diverged from FIFO after step {i} — wrong victim identity");
        }
        assert_eq!(len, 2, "cap of two masks must hold exactly two");
        assert_eq!((hits, misses, evictions), (0, 6, 4), "every revisit was already evicted → all misses");
        // Cross-run: the whole per-step victim trace must reproduce bit-for-bit.
        assert_eq!(run().0, trace, "eviction not deterministic across identical runs");
    }
}
