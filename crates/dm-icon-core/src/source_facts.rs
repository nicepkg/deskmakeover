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
            content_bounds: find_content_bounds(c),
            solid_bounds: solid_bounds(c),
            detected_background: try_detect_background(c),
            foreground: foreground_auto(c),
            segmentation: Arc::new(segment_subject(c)),
            transparent_edges: has_transparent_edges(c),
        }
    }
}

// ── fast-gated accessors ────────────────────────────────────────────────────────
// `fast` returns the cached fact; the default (scalar) build recomputes it — the
// determinism reference the cert diffs the `fast` path against. `match sf { .. }`
// keeps `sf` "used" as the scrutinee under scalar (no unused-arg warning), while the
// cached arm is compiled out.

pub(crate) fn content_bounds(sf: Option<&SourceFacts>, c: &Raster) -> ContentBounds {
    match sf {
        #[cfg(feature = "fast")]
        Some(sf) => sf.content_bounds,
        _ => find_content_bounds(c),
    }
}

pub(crate) fn solid_bounds_of(sf: Option<&SourceFacts>, c: &Raster) -> Option<ContentBounds> {
    match sf {
        #[cfg(feature = "fast")]
        Some(sf) => sf.solid_bounds,
        _ => solid_bounds(c),
    }
}

pub(crate) fn detected_background(sf: Option<&SourceFacts>, c: &Raster) -> Option<Rgba> {
    match sf {
        #[cfg(feature = "fast")]
        Some(sf) => sf.detected_background,
        _ => try_detect_background(c),
    }
}

pub(crate) fn foreground(sf: Option<&SourceFacts>, c: &Raster) -> Option<ContentBounds> {
    match sf {
        #[cfg(feature = "fast")]
        Some(sf) => sf.foreground,
        _ => foreground_auto(c),
    }
}

pub(crate) fn transparent_edges(sf: Option<&SourceFacts>, c: &Raster) -> bool {
    match sf {
        #[cfg(feature = "fast")]
        Some(sf) => sf.transparent_edges,
        _ => has_transparent_edges(c),
    }
}

/// The cached segmentation (`.mask` / `.field`), or a fresh one under scalar. Returns
/// an `Arc` so the mask is shared read-only without a 64 KB clone on the hot path.
pub(crate) fn segmentation(sf: Option<&SourceFacts>, c: &Raster) -> Arc<Segmentation> {
    match sf {
        #[cfg(feature = "fast")]
        Some(sf) => Arc::clone(&sf.segmentation),
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
}
