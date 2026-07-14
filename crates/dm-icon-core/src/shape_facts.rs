//! Per-(source, shape) analysis facts (M6 kernel-speed, codex R2 perf #6).
//!
//! `matches_shape(c, shape)` (compose `PassthroughMatch` gate) and
//! `max_scale_auto(c, shape)` (the inscribe scale) are pure functions of the source
//! pixels AND the config `shape`, so — unlike the source-only [`crate::source_facts`] —
//! they cannot live in the source analysis bundle (keyed by source digest alone). Each is
//! a full O(pixels) silhouette scan, recomputed on EVERY render even though it is
//! size-independent: an interactive slider-drag re-scans the same source+shape once per
//! frame. `ShapeFacts` caches their final EXACT result, keyed by `(source, shape)` in the
//! `RenderSession`.
//!
//! Byte-safety follows the Phase-2 model exactly (see [`crate::source_facts`]): the
//! accessors are gated on the `fast` feature — `fast` serves the cached value, the default
//! (scalar) build recomputes, and the four-way 1487-cell M6 cert diffs the two. Because
//! both facts are pure functions of the source pixels + shape, a cached value is
//! bit-for-bit a fresh recompute (the `f64` via `to_bits`).
//!
//! Self-heal, and why `max_scale_auto` is safe to serve for a `backdrop_swapped` raster:
//! `backs(c, shape)` gates serving on the raster DIMENSIONS + shape matching. The one
//! non-source raster the inscribe path ever sees is `compose::backdrop_swapped(source,…)`,
//! which mutates RGB only and NEVER alpha; `max_scale_auto` (and `matches_shape`) read
//! ONLY the alpha channel (`find_content_bounds` / `solid_bounds` / `alpha_at`), so the
//! cached source value is bit-identical to a recompute on the swap. That alpha-invariance
//! is pinned by `max_scale_auto_is_invariant_under_rgb_only_mutation`, and the cert
//! (scalar recomputes on the swap, fast serves the source value) is the end-to-end anchor.

use std::collections::HashMap;
use std::hash::Hash;

use crate::analysis::{matches_shape, max_scale_auto};
use crate::raster::Raster;
use crate::shapes::IconShape;

/// Bumped whenever a cached shape-fact algorithm (`matches_shape` / `max_scale_auto`, or
/// any threshold they read) changes — invalidates cached facts across a persisted store
/// (the `RenderSession` folds it into the cache key alongside the source key + shape).
pub const SHAPE_FACTS_SCHEMA_VERSION: u32 = 1;

/// The `(source, shape)` analysis facts of one source under one target shape. Both fields
/// are pure functions of the source pixels + `shape`; [`ShapeFacts::compute`] runs each
/// exactly as the compose path would, so a cached field is byte-identical to a recompute.
///
/// Fields are read only by the `fast` accessors below; under the scalar reference build the
/// accessors recompute and never touch them.
#[cfg_attr(not(feature = "fast"), allow(dead_code))]
pub struct ShapeFacts {
    /// Dimensions + shape these facts were computed for. The accessors serve a cached fact
    /// only when the raster + shape they are asked about match — otherwise they self-heal
    /// (recompute). See `backs`.
    raster_dims: (usize, usize),
    shape: IconShape,
    matches_shape: bool,
    max_scale_auto: f64,
}

impl ShapeFacts {
    /// Compute both shape facts once. Pure — depends only on `c`'s pixels and `shape`.
    pub fn compute(c: &Raster, shape: IconShape) -> Self {
        Self {
            raster_dims: (c.width, c.height),
            shape,
            matches_shape: matches_shape(c, shape),
            max_scale_auto: max_scale_auto(c, shape),
        }
    }

    /// True when these facts back a raster of `c`'s dimensions under `shape`. When false the
    /// caller asked about a raster/shape these facts were not computed for (a re-registered
    /// source of a different size, or the `backdrop_swapped` raster is fine — same dims), so
    /// the accessors self-heal (recompute) instead of serving a mismatched fact.
    #[cfg(feature = "fast")]
    fn backs(&self, c: &Raster, shape: IconShape) -> bool {
        self.raster_dims == (c.width, c.height) && self.shape == shape
    }
}

// ── fast-gated accessors ────────────────────────────────────────────────────────
// `fast` returns the cached fact WHEN it backs `(c, shape)`; otherwise — and always under
// the default (scalar) build — it recomputes, the determinism reference the cert diffs the
// `fast` path against. `match sf { .. }` keeps `sf` "used" under scalar (the cached arm is
// compiled out).

pub(crate) fn matches_shape_of(sf: Option<&ShapeFacts>, c: &Raster, shape: IconShape) -> bool {
    match sf {
        #[cfg(feature = "fast")]
        Some(sf) if sf.backs(c, shape) => sf.matches_shape,
        _ => matches_shape(c, shape),
    }
}

pub(crate) fn max_scale_auto_of(sf: Option<&ShapeFacts>, c: &Raster, shape: IconShape) -> f64 {
    match sf {
        #[cfg(feature = "fast")]
        Some(sf) if sf.backs(c, shape) => sf.max_scale_auto,
        _ => max_scale_auto(c, shape),
    }
}

/// A session-owned cache of `(key, shape) → ShapeFacts`, schema-stamped. Lives here (not
/// inlined in `RenderSession`) so the whole shape-fact concern — keying, staleness, and its
/// byte-identity tests — stays cohesive and render_session.rs holds its 500-line cap. `K` is
/// the caller's source-content key (the `RenderSession`'s `SourceKey`).
pub struct ShapeFactsCache<K: Eq + Hash + Copy> {
    entries: HashMap<(K, IconShape), (u32, ShapeFacts)>,
}

impl<K: Eq + Hash + Copy> Default for ShapeFactsCache<K> {
    fn default() -> Self {
        Self { entries: HashMap::new() }
    }
}

impl<K: Eq + Hash + Copy> ShapeFactsCache<K> {
    pub fn new() -> Self {
        Self::default()
    }

    /// The cached `(key, shape)` facts, computing + caching on a miss or a schema bump.
    /// `raster` MUST be the source `key` identifies. Pure ⇒ a cached fact is byte-identical
    /// to a recompute (proven by `cached_shape_facts_equal_direct_recompute` + the cert).
    pub fn get_or_compute(&mut self, key: K, shape: IconShape, raster: &Raster) -> &ShapeFacts {
        let stale = self
            .entries
            .get(&(key, shape))
            .map_or(true, |(schema, _)| *schema != SHAPE_FACTS_SCHEMA_VERSION);
        if stale {
            self.entries
                .insert((key, shape), (SHAPE_FACTS_SCHEMA_VERSION, ShapeFacts::compute(raster, shape)));
        }
        &self.entries.get(&(key, shape)).expect("just inserted").1
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod shape_facts_cert {
    use super::*;
    use crate::raster::Raster;

    /// A circle-ish solid disc → a source whose silhouette can MATCH a Circle shape and has
    /// a non-trivial inscribe scale.
    fn disc(n: usize) -> Raster {
        let mut r = Raster::new(n, n);
        let c = n as f64 / 2.0;
        let rad = n as f64 * 0.48;
        for y in 0..n {
            for x in 0..n {
                let dx = x as f64 + 0.5 - c;
                let dy = y as f64 + 0.5 - c;
                if dx * dx + dy * dy <= rad * rad {
                    r.data[(y * n + x) * 4 + 3] = 255;
                }
            }
        }
        r
    }

    /// A centred opaque square → a source that does NOT match Circle but inscribes.
    fn square(n: usize) -> Raster {
        let mut r = Raster::new(n, n);
        let (lo, hi) = (n / 5, n - n / 5);
        for y in lo..hi {
            for x in lo..hi {
                r.data[(y * n + x) * 4 + 3] = 255;
            }
        }
        r
    }

    /// Every cached field must equal a direct recompute — the byte-identity contract the
    /// fast accessors rely on (Win 4 hard gate). `max_scale_auto` via `to_bits`.
    #[test]
    fn cached_shape_facts_equal_direct_recompute() {
        for c in [disc(64), square(64), Raster::new(48, 48)] {
            for shape in [IconShape::Circle, IconShape::Apple, IconShape::Diamond, IconShape::None] {
                let f = ShapeFacts::compute(&c, shape);
                assert_eq!(f.matches_shape, matches_shape(&c, shape), "matches_shape {shape:?}");
                assert_eq!(
                    f.max_scale_auto.to_bits(),
                    max_scale_auto(&c, shape).to_bits(),
                    "max_scale_auto bits {shape:?}"
                );
            }
        }
    }

    /// The accessors on `Some(cached)` must equal the accessors on `None` (recompute) — the
    /// cache-off/on differential at the fact level (the RGBA cert covers it end-to-end).
    #[cfg(feature = "fast")]
    #[test]
    fn accessor_cache_on_equals_cache_off() {
        let c = disc(64);
        for shape in [IconShape::Circle, IconShape::Diamond, IconShape::None] {
            let f = ShapeFacts::compute(&c, shape);
            assert_eq!(matches_shape_of(Some(&f), &c, shape), matches_shape_of(None, &c, shape));
            assert_eq!(
                max_scale_auto_of(Some(&f), &c, shape).to_bits(),
                max_scale_auto_of(None, &c, shape).to_bits()
            );
        }
    }

    /// Self-heal: facts for one shape/size, asked about a DIFFERENT shape or size, must
    /// recompute (== the `None` path), never serve a mismatched fact.
    #[cfg(feature = "fast")]
    #[test]
    fn accessor_self_heals_on_shape_or_size_mismatch() {
        let c = disc(64);
        let for_circle = ShapeFacts::compute(&c, IconShape::Circle);
        // Different shape on the same raster → recompute.
        assert_eq!(
            matches_shape_of(Some(&for_circle), &c, IconShape::Diamond),
            matches_shape_of(None, &c, IconShape::Diamond)
        );
        assert_eq!(
            max_scale_auto_of(Some(&for_circle), &c, IconShape::Diamond).to_bits(),
            max_scale_auto_of(None, &c, IconShape::Diamond).to_bits()
        );
        // Different size (a re-registered smaller raster) under the SAME shape → recompute.
        let small = disc(32);
        assert_eq!(
            matches_shape_of(Some(&for_circle), &small, IconShape::Circle),
            matches_shape_of(None, &small, IconShape::Circle)
        );
        assert_eq!(
            max_scale_auto_of(Some(&for_circle), &small, IconShape::Circle).to_bits(),
            max_scale_auto_of(None, &small, IconShape::Circle).to_bits()
        );
    }

    /// The alpha-invariance the `backdrop_swapped` serve relies on: `max_scale_auto` (and
    /// `matches_shape`) read ONLY the alpha channel, so mutating RGB while leaving alpha
    /// untouched — exactly what `compose::backdrop_swapped` does — cannot change either
    /// value. This is why serving the SOURCE's cached fact for a swapped raster (same dims)
    /// is byte-exact.
    #[test]
    fn max_scale_auto_is_invariant_under_rgb_only_mutation() {
        let mut src = disc(80);
        // Paint RGB into every pixel WITHOUT touching alpha (mirrors backdrop_swapped).
        for i in 0..src.width * src.height {
            src.data[i * 4] = 17;
            src.data[i * 4 + 1] = 200;
            src.data[i * 4 + 2] = 99;
        }
        let mut swapped = src.clone();
        for i in 0..swapped.width * swapped.height {
            swapped.data[i * 4] = 240; // recolour RGB only
            swapped.data[i * 4 + 1] = 12;
            swapped.data[i * 4 + 2] = 210;
            // alpha (i*4+3) deliberately left as-is
        }
        for shape in [IconShape::Circle, IconShape::Diamond, IconShape::Pebble, IconShape::None] {
            assert_eq!(matches_shape(&src, shape), matches_shape(&swapped, shape), "matches_shape {shape:?}");
            assert_eq!(
                max_scale_auto(&src, shape).to_bits(),
                max_scale_auto(&swapped, shape).to_bits(),
                "max_scale_auto bits {shape:?}"
            );
        }
    }

    /// The session cache must REUSE per `(key, shape)` (one entry across repeats, value ==
    /// recompute) and key DISTINCT entries for a different shape or a different source key.
    #[test]
    fn cache_reuses_per_key_shape_and_is_bit_exact() {
        let c = disc(64);
        let mut cache: ShapeFactsCache<u8> = ShapeFactsCache::new();
        for _ in 0..4 {
            let f = cache.get_or_compute(1u8, IconShape::Circle, &c);
            assert_eq!(f.matches_shape, matches_shape(&c, IconShape::Circle));
            assert_eq!(f.max_scale_auto.to_bits(), max_scale_auto(&c, IconShape::Circle).to_bits());
        }
        assert_eq!(cache.len(), 1, "same (key, shape) must reuse one entry across 4 calls");
        let _ = cache.get_or_compute(1u8, IconShape::Diamond, &c); // different shape
        assert_eq!(cache.len(), 2, "a different shape must key a distinct entry");
        let _ = cache.get_or_compute(2u8, IconShape::Circle, &c); // different source key
        assert_eq!(cache.len(), 3, "a different source key must key a distinct entry");
    }
}
