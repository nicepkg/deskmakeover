//! Immutable per-source analysis facts (M6 kernel-speed Phase 2).
//!
//! The compose path recomputes a handful of PURE functions of the source raster on
//! every render — `find_content_bounds` (~6×/render), `solid_bounds` (~3×),
//! `try_detect_background`, `foreground_auto`, `segment_subject`,
//! `has_transparent_edges`. The frozen TS oracle memoizes these per raster
//! (`analysis.ts` / `segment.ts`); Rust only cached `IconProfile`. This bundles the
//! rest into a `SourceFacts` the `RenderSession` computes once per source (keyed by
//! `source_hash + schema`, same as the profile) and threads into compose.
//!
//! Because every field is a pure function of the source pixels, serving a cached
//! value is BYTE-NEUTRAL. Byte-identity is enforced the Phase-1 way: the accessors
//! are gated on the `fast` feature — `fast` returns the cached fact, the default
//! (scalar) build recomputes it, and the four-way 1487-cell cert diffs the two.
//!
//! Cache key + trust contract: `RenderSession` keys these by the caller-supplied
//! `source_hash + schema`. If a caller REUSES a hash across two different rasters (a
//! contract violation — the real callers don't; see `RenderSession::register`), the
//! accessors SELF-HEAL: each serves the cached fact only when it backs a raster of
//! the current dimensions (`SourceFacts::backs`), otherwise it recomputes. So a
//! same-hash raster of a DIFFERENT size degrades fast to the scalar path (no OOB from
//! a stale larger mask, no silent fork). A same-SIZE different-CONTENT reuse still
//! forks (dimensions match, content differs) — that residual is the documented
//! trust-contract cost, root-fixed in Phase 4 when the key becomes a content digest
//! (BLAKE3 of the source bytes → different image ⇒ different key). Until then, native
//! callers must not reuse a `source_hash` for different bytes.
//!
//! Deliberately EXCLUDED (not pure functions of the source alone — honest
//! annotation, cf. Phase 1's None-shape stamp):
//!   • `matches_shape(c, shape)` / `max_scale_inside(c, shape)` — depend on the
//!     config `shape`, so they are config facts, not source facts. Caching them
//!     needs a `(source, shape)` key, outside the `source digest + schema` contract.
//!   • `find_content_bounds(&layer)` on a DERIVED raster (mono layer) — not the
//!     immutable source, so it stays a direct recompute.

use std::sync::Arc;

use crate::analysis::{
    find_content_bounds, foreground_auto, has_transparent_edges, solid_bounds,
    try_detect_background, ContentBounds,
};
use crate::raster::{Raster, Rgba};
use crate::segment::{segment_subject, Segmentation};

/// Bumped whenever any cached source-fact algorithm changes — invalidates cached
/// facts across a persisted store (the `RenderSession` folds it into the cache key).
pub const SOURCE_FACTS_SCHEMA_VERSION: u32 = 1;

/// The immutable analysis facts of one source raster. Every field is a pure
/// function of the source pixels; `compute` runs each exactly as the compose path
/// would, so a cached field is byte-identical to a fresh recompute.
///
/// Fields are read only by the `fast` accessors below; under the scalar reference
/// build the accessors recompute and never touch them.
#[cfg_attr(not(feature = "fast"), allow(dead_code))]
pub struct SourceFacts {
    /// Dimensions of the raster these facts were computed for. The accessors serve a
    /// cached fact only when the raster they are asked about matches — otherwise they
    /// self-heal (recompute). See `backs`.
    raster_dims: (usize, usize),
    content_bounds: ContentBounds,
    solid_bounds: Option<ContentBounds>,
    detected_background: Option<Rgba>,
    foreground: Option<ContentBounds>,
    segmentation: Arc<Segmentation>,
    transparent_edges: bool,
}

impl SourceFacts {
    /// Compute every source fact once. Pure — depends only on `c`'s pixels.
    pub fn compute(c: &Raster) -> Self {
        Self {
            raster_dims: (c.width, c.height),
            content_bounds: find_content_bounds(c),
            solid_bounds: solid_bounds(c),
            detected_background: try_detect_background(c),
            foreground: foreground_auto(c),
            segmentation: Arc::new(segment_subject(c)),
            transparent_edges: has_transparent_edges(c),
        }
    }

    /// True when these facts back a raster of `c`'s dimensions. When false the caller
    /// reused a `source_hash` across differently sized rasters — a documented trust-
    /// contract violation (see `RenderSession::register`) — and the accessors below
    /// self-heal (recompute) instead of serving stale facts. This is the guard that
    /// keeps the fast build from indexing a stale, larger cached segmentation mask
    /// into a smaller raster in `mono_subject_layer` (an OOB panic; scalar already
    /// recomputes, so this realigns fast with the scalar reference).
    #[cfg(feature = "fast")]
    fn backs(&self, c: &Raster) -> bool {
        self.raster_dims == (c.width, c.height)
    }
}

// ── fast-gated accessors ────────────────────────────────────────────────────────
// `fast` returns the cached fact WHEN it backs `c` (same dimensions); otherwise — and
// always under the default (scalar) build — it recomputes, the determinism reference
// the cert diffs the `fast` path against. The `if sf.backs(c)` guard self-heals a
// reused-hash contract violation (see `SourceFacts::backs`): a stale fact for a
// differently sized raster degrades to a recompute rather than a silent fork or OOB.
// `match sf { .. }` keeps `sf` "used" as the scrutinee under scalar (no unused-arg
// warning), while the cached arm is compiled out.

pub(crate) fn content_bounds(sf: Option<&SourceFacts>, c: &Raster) -> ContentBounds {
    match sf {
        #[cfg(feature = "fast")]
        Some(sf) if sf.backs(c) => sf.content_bounds,
        _ => find_content_bounds(c),
    }
}

pub(crate) fn solid_bounds_of(sf: Option<&SourceFacts>, c: &Raster) -> Option<ContentBounds> {
    match sf {
        #[cfg(feature = "fast")]
        Some(sf) if sf.backs(c) => sf.solid_bounds,
        _ => solid_bounds(c),
    }
}

pub(crate) fn detected_background(sf: Option<&SourceFacts>, c: &Raster) -> Option<Rgba> {
    match sf {
        #[cfg(feature = "fast")]
        Some(sf) if sf.backs(c) => sf.detected_background,
        _ => try_detect_background(c),
    }
}

pub(crate) fn foreground(sf: Option<&SourceFacts>, c: &Raster) -> Option<ContentBounds> {
    match sf {
        #[cfg(feature = "fast")]
        Some(sf) if sf.backs(c) => sf.foreground,
        _ => foreground_auto(c),
    }
}

pub(crate) fn transparent_edges(sf: Option<&SourceFacts>, c: &Raster) -> bool {
    match sf {
        #[cfg(feature = "fast")]
        Some(sf) if sf.backs(c) => sf.transparent_edges,
        _ => has_transparent_edges(c),
    }
}

/// The cached segmentation (`.mask` / `.field`), or a fresh one under scalar / on a
/// dimension-mismatch self-heal. Returns an `Arc` so the mask is shared read-only
/// without a 64 KB clone on the hot path. The `backs(c)` guard is load-bearing here:
/// the returned mask is always sized to `c`, so `mono_subject_layer` never indexes a
/// stale larger mask into a smaller raster.
pub(crate) fn segmentation(sf: Option<&SourceFacts>, c: &Raster) -> Arc<Segmentation> {
    match sf {
        #[cfg(feature = "fast")]
        Some(sf) if sf.backs(c) => Arc::clone(&sf.segmentation),
        _ => Arc::new(segment_subject(c)),
    }
}

#[cfg(all(test, feature = "fast"))]
mod facts_cert {
    use super::*;
    use crate::analysis::{bounds_h, bounds_w};

    // A few representative sources: a floating dot, a solid square, a plated icon.
    fn dot() -> Raster {
        let mut r = Raster::new(64, 64);
        for y in 20..44 {
            for x in 20..44 {
                let i4 = (y * 64 + x) * 4;
                r.data[i4] = 200;
                r.data[i4 + 1] = 60;
                r.data[i4 + 2] = 40;
                r.data[i4 + 3] = 255;
            }
        }
        r
    }

    fn plated() -> Raster {
        // Solid teal plate with a darker centred blob (a detectable own background).
        let mut r = Raster::new(64, 64);
        for i in 0..64 * 64 {
            r.data[i * 4] = 20;
            r.data[i * 4 + 1] = 140;
            r.data[i * 4 + 2] = 160;
            r.data[i * 4 + 3] = 255;
        }
        for y in 24..40 {
            for x in 24..40 {
                let i4 = (y * 64 + x) * 4;
                r.data[i4] = 240;
                r.data[i4 + 1] = 240;
                r.data[i4 + 2] = 240;
            }
        }
        r
    }

    /// Every cached field must equal a direct recompute — the byte-identity contract
    /// the fast accessors rely on (review Phase 2 hard gate).
    #[test]
    fn cached_facts_equal_direct_recompute() {
        for c in [dot(), plated(), Raster::new(48, 48)] {
            let f = SourceFacts::compute(&c);
            assert_eq!(f.content_bounds, find_content_bounds(&c), "content_bounds");
            assert_eq!(f.solid_bounds, solid_bounds(&c), "solid_bounds");
            assert_eq!(f.detected_background, try_detect_background(&c), "detected_background");
            assert_eq!(f.foreground, foreground_auto(&c), "foreground");
            assert_eq!(f.transparent_edges, has_transparent_edges(&c), "transparent_edges");
            let seg = segment_subject(&c);
            assert_eq!(f.segmentation.mask, seg.mask, "segmentation.mask");
            assert_eq!(f.segmentation.field, seg.field, "segmentation.field");
            assert_eq!(f.segmentation.mode, seg.mode, "segmentation.mode");
        }
    }

    /// The accessors on `Some(cached)` must equal the accessors on `None` (recompute)
    /// — the cache-off/on differential at the fact level (the RGBA cert covers it
    /// end-to-end; this pins each fact).
    #[test]
    fn accessor_cache_on_equals_cache_off() {
        let c = plated();
        let f = SourceFacts::compute(&c);
        assert_eq!(content_bounds(Some(&f), &c), content_bounds(None, &c));
        assert_eq!(solid_bounds_of(Some(&f), &c), solid_bounds_of(None, &c));
        assert_eq!(detected_background(Some(&f), &c), detected_background(None, &c));
        assert_eq!(foreground(Some(&f), &c), foreground(None, &c));
        assert_eq!(transparent_edges(Some(&f), &c), transparent_edges(None, &c));
        let on = segmentation(Some(&f), &c);
        let off = segmentation(None, &c);
        assert_eq!(on.mask, off.mask);
        assert_eq!(on.field, off.field);
        // Sanity: the plate fixture actually exercises non-trivial facts.
        assert!(bounds_w(content_bounds(Some(&f), &c)) > 0 && bounds_h(content_bounds(Some(&f), &c)) > 0);
    }

    /// Self-heal on a reused-hash contract violation: facts computed for one raster,
    /// served for a DIFFERENTLY SIZED raster, must recompute (== the `None` path) —
    /// never serve stale facts. The load-bearing case is `segmentation`: the served
    /// mask must be sized to the CURRENT raster, so `mono_subject_layer` can't index a
    /// stale larger mask into a smaller raster (a fast-only OOB). This is the fact-
    /// level proof of the scenario the session-level test drives end-to-end.
    #[test]
    fn accessor_self_heals_on_dim_mismatch() {
        let stale = SourceFacts::compute(&plated()); // backs a 64×64 raster
        let small = Raster::new(32, 32); // different size → `stale` must not be served
        assert_eq!(content_bounds(Some(&stale), &small), content_bounds(None, &small));
        assert_eq!(solid_bounds_of(Some(&stale), &small), solid_bounds_of(None, &small));
        assert_eq!(detected_background(Some(&stale), &small), detected_background(None, &small));
        assert_eq!(foreground(Some(&stale), &small), foreground(None, &small));
        assert_eq!(transparent_edges(Some(&stale), &small), transparent_edges(None, &small));
        let healed = segmentation(Some(&stale), &small);
        assert_eq!(healed.mask, segmentation(None, &small).mask);
        assert_eq!(healed.mask.len(), small.width * small.height, "healed mask must fit the CURRENT raster");
    }
}

/// Purity guard for the cache key. `SourceFacts` is cached by `source_hash +
/// SOURCE_FACTS_SCHEMA_VERSION`; that key is sound ONLY while every analysis
/// threshold is a compile-time constant (never an `IconConfig` field) — a
/// config-derived threshold would serve one config's fact for another source-hash
/// twin. Purity was audited: all of the thresholds below are `const`/literal.
///
/// The AUTHORITATIVE anchor is the frozen-TS-golden 1487-cell cert: change any
/// threshold value and the Rust output diverges from the golden and the cert turns
/// red; make one config-dependent and the same-source/multi-config cells make the
/// four-way fast-vs-scalar differential diverge. This module additionally pins the
/// one *named* source-fact constant and documents the inline ones as the
/// schema-bump checklist. Change ANY of these ⇒ bump `SOURCE_FACTS_SCHEMA_VERSION`:
///
/// | fact                       | threshold             | value | site                    |
/// |----------------------------|-----------------------|-------|-------------------------|
/// | solid_bounds / foreground  | `SOLID_ALPHA`         | 128   | analysis/mod.rs         |
/// | find_content_bounds        | alpha cutoff `> 24`   | 24    | analysis/mod.rs         |
/// | foreground_auto            | bg tolerance          | 48    | analysis/mod.rs         |
/// | try_canvas_background       | rect-ring tolerance   | 18    | analysis/background.rs  |
/// | try_shape_background        | shape-ring tolerance  | 24    | analysis/background.rs  |
/// | try_shape_background        | opaque coverage       | 0.62  | analysis/background.rs  |
#[cfg(test)]
mod thresholds {
    use crate::analysis::SOLID_ALPHA;

    #[test]
    fn solid_alpha_threshold_is_frozen() {
        // Guards the one named source-fact constant. Changing it (or any inline
        // threshold in the module docs above) must bump SOURCE_FACTS_SCHEMA_VERSION;
        // the frozen-golden corpus cert is the behavioral anchor for all of them.
        assert_eq!(SOLID_ALPHA, 128);
    }
}
