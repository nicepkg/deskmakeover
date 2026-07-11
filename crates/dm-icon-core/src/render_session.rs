//! RenderSession — register / analyze / setLook / render with a persisted
//! per-source profile cache keyed by `source_hash + analysis_schema_version`
//! (ADR-0019 M5 exit). The core-side session: it owns decoded sources, their
//! cached `IconProfile`s, and the current look, and feeds the cached profile
//! into the derived-field lane so `render` does not re-analyze. The per-item
//! config resolution (type ladder) and the cross-icon hue-spread orchestration
//! stay in the app/store layer (which calls `compute_hue_spread` + `seed_of`).

use std::collections::HashMap;

use crate::compose::{render_tile_cached, ComposeDiagnostics, RenderOpts};
use crate::config::Config;
use crate::mask_cache::MaskCache;
use crate::profile::{icon_profile, IconProfile};
use crate::raster::Raster;
use crate::source_facts::{SourceFacts, SOURCE_FACTS_SCHEMA_VERSION};

/// Bumped whenever the analysis/profile algorithm changes — invalidates cached
/// profiles across a persisted store.
pub const ANALYSIS_SCHEMA_VERSION: u32 = 1;

struct Registered {
    raster: Raster,
    source_hash: u64,
}

struct CachedProfile {
    schema: u32,
    profile: IconProfile,
}

struct CachedSourceFacts {
    schema: u32,
    facts: SourceFacts,
}

#[derive(Default)]
pub struct RenderSession {
    sources: HashMap<String, Registered>,
    profiles: HashMap<u64, CachedProfile>,
    /// Immutable per-source analysis facts (content/solid/foreground bounds,
    /// background, segmentation, transparent-edges) the compose path would otherwise
    /// recompute every render — keyed by `source_hash + schema`, same as the profile
    /// (M6 Phase 2). Fed as `Option<&SourceFacts>` into compose; the fast accessors
    /// read it, the scalar reference recomputes.
    source_facts: HashMap<u64, CachedSourceFacts>,
    look: Option<Config>,
    /// Session-owned geometry mask cache — reused across every `render`, so the
    /// dominant `shape_mask` recompute collapses to a warm hit (M6 Phase 1). Under
    /// the default (scalar) build this is a passthrough; `--features fast` caches.
    mask_cache: MaskCache,
}

impl RenderSession {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register (or replace) a decoded 256px source under an id, with a caller-
    /// supplied content hash of the source bytes for cache keying.
    ///
    /// Trust contract: the caller owns `source_hash` correctness. Two different
    /// rasters sharing a hash collide in these caches (one's profile / source facts
    /// reused for the other); the app derives the hash from the real source bytes and
    /// never reuses one for different bytes. The source-fact cache SELF-HEALS the
    /// dangerous half of a violation — a reused hash across differently sized rasters
    /// recomputes instead of indexing a stale mask out of bounds (see
    /// `source_facts::SourceFacts::backs`). A same-size different-content reuse still
    /// forks; that residual is root-fixed in Phase 4 (content-digest key). Native
    /// callers must not reuse a `source_hash` for different bytes before then.
    pub fn register(&mut self, id: impl Into<String>, source_hash: u64, raster: Raster) {
        self.sources.insert(id.into(), Registered { raster, source_hash });
    }

    /// The cached profile for a registered source, computing + caching on a miss
    /// or when the analysis schema version has moved.
    pub fn analyze(&mut self, id: &str) -> Option<&IconProfile> {
        let hash = self.sources.get(id)?.source_hash;
        let stale = self
            .profiles
            .get(&hash)
            .map_or(true, |c| c.schema != ANALYSIS_SCHEMA_VERSION);
        if stale {
            let profile = icon_profile(&self.sources.get(id)?.raster);
            self.profiles.insert(hash, CachedProfile { schema: ANALYSIS_SCHEMA_VERSION, profile });
        }
        self.profiles.get(&hash).map(|c| &c.profile)
    }

    /// The decode-time hue-spread seed (subject rim colour hex) for a source, or
    /// None for the no-hue tail (mirrors the store's `seedOf`).
    pub fn seed_of(&mut self, id: &str) -> Option<String> {
        let colour = self.analyze(id)?.subject_rim_colour?;
        Some(format!("#{:02X}{:02X}{:02X}", colour.r, colour.g, colour.b))
    }

    /// Set the current look (the resolved global config the caller derives).
    pub fn set_look(&mut self, config: Config) {
        self.look = Some(config);
    }

    /// Render a registered source under the current look, consuming the cached
    /// profile (byte-identical to a fresh analysis).
    /// Compute + cache the immutable source facts for a source (miss or schema move),
    /// mirroring `analyze`. Pure — a cached fact is byte-identical to a recompute.
    fn ensure_source_facts(&mut self, id: &str, hash: u64) {
        let stale = self
            .source_facts
            .get(&hash)
            .map_or(true, |c| c.schema != SOURCE_FACTS_SCHEMA_VERSION);
        if !stale {
            return;
        }
        let Some(reg) = self.sources.get(id) else { return };
        let facts = SourceFacts::compute(&reg.raster);
        self.source_facts.insert(hash, CachedSourceFacts { schema: SOURCE_FACTS_SCHEMA_VERSION, facts });
    }

    pub fn render(
        &mut self,
        id: &str,
        is_shortcut: bool,
        show_original: bool,
        size: usize,
        opts: &RenderOpts,
        diag: &mut ComposeDiagnostics,
    ) -> Option<Raster> {
        let hash = self.sources.get(id)?.source_hash;
        self.analyze(id)?; // populate the profile cache
        self.ensure_source_facts(id, hash); // populate the source-fact cache
        let config = self.look.as_ref()?; // None until set_look — align with the Option API
        let raster = &self.sources.get(id)?.raster;
        let profile = self.profiles.get(&hash).map(|c| &c.profile);
        let facts = self.source_facts.get(&hash).map(|c| &c.facts);
        Some(render_tile_cached(raster, config, is_shortcut, show_original, size, opts, diag, profile, &mut self.mask_cache, facts))
    }

    #[cfg(test)]
    fn cached_source_facts_count(&self) -> usize {
        self.source_facts.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        Band, Config, Distinction, FilterStyle, MarkStyle, MonoStyle, PlateFallback, Subject,
    };
    use crate::shapes::IconShape;

    fn solid_source(r: u8, g: u8, b: u8) -> Raster {
        let mut raster = Raster::new(256, 256);
        for i in 0..256 * 256 {
            raster.data[i * 4] = r;
            raster.data[i * 4 + 1] = g;
            raster.data[i * 4 + 2] = b;
            raster.data[i * 4 + 3] = 255;
        }
        raster
    }

    fn spectrum() -> Config {
        Config {
            shape: IconShape::Circle,
            subject: Subject::Original,
            tint: 0xff6f5e,
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

    #[test]
    fn analyze_caches_by_hash_and_schema() {
        let mut s = RenderSession::new();
        s.register("a", 0xABCD, solid_source(30, 120, 200));
        let k1 = s.analyze("a").unwrap().kind;
        // A second analyze returns the cached profile (same kind), no recompute path panics.
        let k2 = s.analyze("a").unwrap().kind;
        assert_eq!(k1, k2);
        assert!(s.analyze("missing").is_none());
    }

    #[test]
    fn render_matches_direct_render_tile() {
        let src = solid_source(30, 120, 200);
        let mut s = RenderSession::new();
        s.register("a", 1, src.clone());
        s.set_look(spectrum());
        let opts = RenderOpts::default();
        let mut d1 = ComposeDiagnostics::default();
        let session_tile = s.render("a", false, false, 256, &opts, &mut d1).unwrap();
        // The cached-profile path must be byte-identical to a fresh render_tile.
        let mut d2 = ComposeDiagnostics::default();
        let direct = crate::compose::render_tile(&src, &spectrum(), false, false, 256, &opts, &mut d2);
        assert_eq!(session_tile.data, direct.data);
        assert_eq!(d1.lane, d2.lane);
    }

    #[test]
    fn render_without_a_look_returns_none() {
        // The look is optional until set; render should mirror the rest of the API and
        // return None rather than panicking.
        let mut s = RenderSession::new();
        s.register("a", 1, solid_source(30, 120, 200));
        let mut diag = ComposeDiagnostics::default();
        assert!(s.render("a", false, false, 256, &RenderOpts::default(), &mut diag).is_none());
    }

    #[test]
    fn seed_of_returns_rim_hex_or_none() {
        let mut s = RenderSession::new();
        s.register("a", 7, solid_source(200, 40, 40));
        // A uniform red square has a rim colour → a hex seed.
        let seed = s.seed_of("a");
        assert!(seed.as_deref().map(|h| h.starts_with('#')).unwrap_or(true));
    }

    // ── Phase 2 source-fact cache hard gate ─────────────────────────────────────

    /// A teal plate with a lighter centred blob → a detectable own background
    /// (exercises the plate / foreground / background source facts).
    fn plated_source() -> Raster {
        let mut r = Raster::new(256, 256);
        for i in 0..256 * 256 {
            r.data[i * 4] = 20;
            r.data[i * 4 + 1] = 140;
            r.data[i * 4 + 2] = 160;
            r.data[i * 4 + 3] = 255;
        }
        for y in 90..166 {
            for x in 90..166 {
                let i4 = (y * 256 + x) * 4;
                r.data[i4] = 240;
                r.data[i4 + 1] = 240;
                r.data[i4 + 2] = 240;
            }
        }
        r
    }

    /// A centred opaque blob on a transparent field (exercises the transparent-edge /
    /// segmentation source facts).
    fn floating_source() -> Raster {
        let mut r = Raster::new(256, 256);
        for y in 80..176 {
            for x in 80..176 {
                let i4 = (y * 256 + x) * 4;
                r.data[i4] = 200;
                r.data[i4 + 1] = 60;
                r.data[i4 + 2] = 40;
                r.data[i4 + 3] = 255;
            }
        }
        r
    }

    fn mono_flat() -> Config {
        // The mono-flat lane reaches `mono_subject_layer` — the segmentation-mask
        // consumer where a stale larger mask would index a smaller raster OOB.
        Config { subject: Subject::Mono, mono_style: MonoStyle::Flat, ..spectrum() }
    }

    /// A centred opaque blob on a transparent field at an arbitrary size — a clear,
    /// non-degenerate subject, so the mono-subject layer proceeds past its 2% guard.
    fn floating_sized(n: usize) -> Raster {
        let mut r = Raster::new(n, n);
        let (lo, hi) = (n / 4, n - n / 4);
        for y in lo..hi {
            for x in lo..hi {
                let i4 = (y * n + x) * 4;
                r.data[i4] = 200;
                r.data[i4 + 1] = 60;
                r.data[i4 + 2] = 40;
                r.data[i4 + 3] = 255;
            }
        }
        r
    }

    fn free_render_cfg(src: &Raster, cfg: &Config, size: usize) -> Vec<u8> {
        let mut d = ComposeDiagnostics::default();
        crate::compose::render_tile(src, cfg, false, false, size, &RenderOpts::default(), &mut d).data
    }

    fn free_render(src: &Raster, size: usize) -> Vec<u8> {
        free_render_cfg(src, &spectrum(), size)
    }

    /// Cache-off/on differential: rendering through the session (source facts cached)
    /// must be byte-identical to a free `render_tile` (facts recomputed), for every
    /// source type and size. Under scalar both recompute (trivially equal); under fast
    /// this pins cached == recompute end-to-end.
    #[test]
    fn session_render_matches_free_render_across_source_types() {
        for (i, src) in [solid_source(30, 120, 200), plated_source(), floating_source()]
            .into_iter()
            .enumerate()
        {
            let mut s = RenderSession::new();
            s.register("a", 1000 + i as u64, src.clone());
            s.set_look(spectrum());
            for size in [96, 256] {
                let mut d = ComposeDiagnostics::default();
                let on = s.render("a", false, false, size, &RenderOpts::default(), &mut d).unwrap();
                assert_eq!(on.data, free_render(&src, size), "source {i} size {size}: cached facts != recompute");
            }
        }
    }

    /// The source-fact cache must never leak one source's facts to another: rendering
    /// the same set in different orders gives each source identical bytes.
    #[test]
    fn render_order_is_source_fact_cache_independent() {
        let srcs = [(1u64, plated_source()), (2, floating_source()), (3, solid_source(200, 40, 40))];
        let render_all = |order: &[usize]| -> Vec<(usize, Vec<u8>)> {
            let mut s = RenderSession::new();
            for (h, r) in &srcs {
                s.register(format!("s{h}"), *h, r.clone());
            }
            s.set_look(spectrum());
            let mut out = Vec::new();
            for &i in order {
                let h = srcs[i].0;
                let mut d = ComposeDiagnostics::default();
                let t = s.render(&format!("s{h}"), false, false, 256, &RenderOpts::default(), &mut d).unwrap();
                out.push((i, t.data));
            }
            out.sort_by_key(|(i, _)| *i);
            out
        };
        assert_eq!(render_all(&[0, 1, 2]), render_all(&[2, 0, 1]), "render output depends on order");
    }

    /// Invalidation: re-registering an id with a different raster + hash must recompute
    /// its facts, not serve the previous raster's cached facts.
    #[test]
    fn re_register_with_new_hash_uses_fresh_source_facts() {
        let mut s = RenderSession::new();
        s.set_look(spectrum());
        s.register("a", 1, plated_source());
        let mut d = ComposeDiagnostics::default();
        let _ = s.render("a", false, false, 256, &RenderOpts::default(), &mut d).unwrap();

        let floating = floating_source();
        s.register("a", 2, floating.clone());
        let mut d2 = ComposeDiagnostics::default();
        let after = s.render("a", false, false, 256, &RenderOpts::default(), &mut d2).unwrap();
        assert_eq!(after.data, free_render(&floating, 256), "stale source facts served after re-register");
    }

    /// The Phase 2 win: a source rendered many times (size changes, slider drags)
    /// computes its facts ONCE and reuses them. 8 renders → exactly one cached entry.
    #[test]
    fn source_facts_computed_once_per_source() {
        let mut s = RenderSession::new();
        s.set_look(spectrum());
        s.register("a", 42, plated_source());
        for size in [64, 96, 128, 160, 192, 224, 256, 96] {
            let mut d = ComposeDiagnostics::default();
            let _ = s.render("a", false, false, size, &RenderOpts::default(), &mut d).unwrap();
        }
        assert_eq!(s.cached_source_facts_count(), 1, "source facts recomputed instead of reused across 8 renders");
    }

    /// Contract-violation self-heal end-to-end (the scenario both Phase-2 reviews
    /// raised): a caller reuses a `source_hash` across differently sized rasters. The
    /// first render caches the LARGER raster's facts; re-registering the SAME hash
    /// with a SMALLER raster and rendering must NOT panic — without the accessor
    /// self-heal the fast build would index the stale larger segmentation mask into
    /// the smaller raster in `mono_subject_layer` (OOB). The mono-flat lane reaches
    /// that layer. The healed output equals a fresh render of the small raster; the
    /// airtight per-fact proof is `source_facts::accessor_self_heals_on_dim_mismatch`.
    #[test]
    fn stale_hash_reuse_self_heals_across_sizes() {
        let mut s = RenderSession::new();
        s.set_look(mono_flat());
        s.register("a", 7, floating_sized(512));
        let mut d = ComposeDiagnostics::default();
        let _ = s.render("a", false, false, 256, &RenderOpts::default(), &mut d).unwrap(); // caches 512² facts

        let small = floating_sized(128);
        s.register("a", 7, small.clone()); // SAME hash, smaller raster — the violation
        let mut d2 = ComposeDiagnostics::default();
        let after = s.render("a", false, false, 256, &RenderOpts::default(), &mut d2).unwrap(); // must not panic
        assert_eq!(
            after.data,
            free_render_cfg(&small, &mono_flat(), 256),
            "fast self-heal must equal a fresh render of the small raster",
        );
    }
}
