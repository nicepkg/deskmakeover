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
use crate::profile::IconProfile;
use crate::raster::Raster;
use crate::render_scratch::RenderScratch;
use crate::source_facts::{build_analysis_bundle, AnalysisBundle, SOURCE_FACTS_SCHEMA_VERSION};

/// Bumped whenever the analysis/profile algorithm changes — invalidates cached
/// profiles across a persisted store.
pub const ANALYSIS_SCHEMA_VERSION: u32 = 1;

/// The per-source cache key. Derived from the raster CONTENT, not the caller's
/// `source_hash`, so a caller that reuses a hash for different bytes cannot alias one
/// source's profile / facts onto another — the root fix for the Phase-2 trust-contract
/// collision (the source-fact `backs` self-heal stays as a belt-and-suspenders second
/// line). Native keys by the full blake3 `source_digest` (necessarily distinct for
/// distinct pixels); wasm keeps its monotonic `nextHash` (expanded), so the shipped
/// .wasm links no blake3 and the preview's collision-free-in-practice behavior is
/// unchanged.
type SourceKey = [u8; 32];

#[cfg(not(target_arch = "wasm32"))]
fn source_key(_caller_hash: u64, raster: &Raster) -> SourceKey {
    crate::output_cache::source_digest(raster)
}

#[cfg(target_arch = "wasm32")]
fn source_key(caller_hash: u64, _raster: &Raster) -> SourceKey {
    let mut k = [0u8; 32];
    k[..8].copy_from_slice(&caller_hash.to_le_bytes());
    k
}

struct Registered {
    raster: Raster,
    key: SourceKey,
}

/// One cache entry per source: the shared analysis bundle (`IconProfile` +
/// `SourceFacts`) built from a SINGLE sub-analysis. The two schema versions are stamped
/// together and BOTH gate staleness (recompute if EITHER moved), so the merged cache
/// stays as correct as the two it replaced.
struct CachedAnalysis {
    profile_schema: u32,
    facts_schema: u32,
    bundle: AnalysisBundle,
}

#[derive(Default)]
pub struct RenderSession {
    sources: HashMap<String, Registered>,
    /// Per-source shared analysis, keyed by the content `SourceKey`. Collapses the
    /// former split profile/source-fact caches into ONE compute (codex R2 C-5): a cold
    /// styled render needs both the `IconProfile` and the immutable `SourceFacts` for
    /// the same raster, and computing them apart ran `segment_subject` (the BFS) and
    /// `try_detect_background` twice. The bundle is a pure function of the pixels, so a
    /// cached entry is byte-identical to a fresh `icon_profile` + `SourceFacts::compute`.
    analyses: HashMap<SourceKey, CachedAnalysis>,
    look: Option<Config>,
    /// Session-owned geometry mask cache — reused across every `render`, so the
    /// dominant `shape_mask` recompute collapses to a warm hit (M6 Phase 1). Under
    /// the default (scalar) build this is a passthrough; `--features fast` caches.
    mask_cache: MaskCache,
    /// Session-owned reusable render scratch (P2-SCRATCH) — the per-size shadow/blur/seat
    /// buffers, reused across every `render` so the interactive slider-drag path stops
    /// allocating them per frame. Byte-neutral: reused buffers are reset to the
    /// fresh-alloc state (see [`RenderScratch`]).
    render_scratch: RenderScratch,
}

impl RenderSession {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register (or replace) a decoded 256px source under an id. `source_hash` is the
    /// caller's advisory content hash; the caches key by a `SourceKey` DERIVED from the
    /// raster (see [`source_key`]), so on native a caller reusing a hash for different
    /// bytes cannot alias — the Phase-2 trust-contract collision is root-fixed here.
    /// The source-fact `backs` self-heal stays as a belt-and-suspenders second line
    /// (and on wasm, where the key is still the caller hash, it remains load-bearing).
    pub fn register(&mut self, id: impl Into<String>, source_hash: u64, raster: Raster) {
        // The pipeline assumes a SQUARE source (the icon canvas); the ring/background analysis uses
        // width for both axes, so a non-square (or zero) raster would panic or misread (audit F7).
        // Real sources — the shell extractor, the PNG decoder, the dev host — are always square, so a
        // non-square one is malformed input: DROP it (the id never resolves → the caller degrades to
        // the original icon) rather than crash. A deeper width/height-aware analysis is deferred.
        if raster.width != raster.height || raster.width == 0 {
            return;
        }
        let key = source_key(source_hash, &raster);
        self.sources.insert(id.into(), Registered { raster, key });
    }

    /// The cached profile for a registered source, computing + caching the shared
    /// analysis bundle on a miss or when either schema version has moved. Standalone
    /// callers (`seed_of`) get the identical profile; the bundle also warms the source
    /// facts, so a subsequent `render` reuses both with zero recompute.
    pub fn analyze(&mut self, id: &str) -> Option<&IconProfile> {
        let key = self.sources.get(id)?.key;
        self.ensure_analysis(id, key)?;
        self.analyses.get(&key).map(|c| &c.bundle.profile)
    }

    /// Compute + cache the shared analysis bundle (profile + source facts) for a source
    /// on a miss or when EITHER schema version has moved (codex R2 C-5). This is the ONE
    /// compute path both `analyze` and `render` go through: `segment_subject` and
    /// `try_detect_background` run ONCE for both the profile and the facts instead of
    /// once each. Pure — a cached bundle is byte-identical to a fresh recompute.
    fn ensure_analysis(&mut self, id: &str, key: SourceKey) -> Option<()> {
        let stale = self.analyses.get(&key).map_or(true, |c| {
            c.profile_schema != ANALYSIS_SCHEMA_VERSION || c.facts_schema != SOURCE_FACTS_SCHEMA_VERSION
        });
        if stale {
            let bundle = build_analysis_bundle(&self.sources.get(id)?.raster);
            self.analyses.insert(
                key,
                CachedAnalysis {
                    profile_schema: ANALYSIS_SCHEMA_VERSION,
                    facts_schema: SOURCE_FACTS_SCHEMA_VERSION,
                    bundle,
                },
            );
        }
        Some(())
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

    /// Render a registered source under the current look, consuming the cached shared
    /// analysis bundle (byte-identical to a fresh analysis).
    pub fn render(
        &mut self,
        id: &str,
        is_shortcut: bool,
        show_original: bool,
        size: usize,
        opts: &RenderOpts,
        diag: &mut ComposeDiagnostics,
    ) -> Option<Raster> {
        let key = self.sources.get(id)?.key;
        // Do NO analysis when its result would be discarded (codex R2 C-5): a no-look render returns
        // None, and `show_original` resamples the source only (render_tile_cached ignores profile +
        // facts on that lane). Analysis is needed ONLY for a styled render. Byte-identical output.
        self.look.as_ref()?; // None until set_look — bail before any analysis
        if !show_original {
            self.ensure_analysis(id, key)?; // ONE shared compute → profile + source facts
        }
        let config = self.look.as_ref()?; // re-borrow for the render (still Some past the guard above)
        let raster = &self.sources.get(id)?.raster;
        let entry = self.analyses.get(&key);
        let profile = entry.map(|c| &c.bundle.profile);
        let facts = entry.map(|c| &c.bundle.facts);
        Some(render_tile_cached(raster, config, is_shortcut, show_original, size, opts, diag, profile, &mut self.mask_cache, &mut self.render_scratch, facts))
    }

    #[cfg(test)]
    fn cached_source_facts_count(&self) -> usize {
        self.analyses.len()
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
    fn non_square_or_zero_sources_are_dropped_not_registered() {
        // audit F7: the analysis assumes a square canvas — a non-square/zero source must be dropped
        // at registration (the id never resolves) rather than reach the width-for-both-axes ring
        // analysis and panic.
        let mut s = RenderSession::new();
        s.register("wide", 0x1, Raster::new(256, 128));
        s.register("tall", 0x2, Raster::new(128, 256));
        s.register("zero", 0x3, Raster::new(0, 0));
        assert!(s.analyze("wide").is_none(), "a non-square source must not register");
        assert!(s.analyze("tall").is_none());
        assert!(s.analyze("zero").is_none());
        // A square source still registers + analyzes.
        s.register("ok", 0x4, solid_source(10, 20, 30));
        assert!(s.analyze("ok").is_some());
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

    /// Contract-violation end-to-end (the scenario both Phase-2 reviews raised): a
    /// caller reuses a `source_hash` across differently sized rasters. Since Phase 4a
    /// the cache keys by content digest, so the smaller raster is a correct MISS (not a
    /// stale hit) — it renders fresh and must NOT panic. The self-heal `backs` is now a
    /// belt-and-suspenders second line that no longer fires here (its airtight per-fact
    /// proof is `source_facts::accessor_self_heals_on_dim_mismatch`, and it stays
    /// load-bearing on wasm, which still keys by the caller hash). The output equals a
    /// fresh render of the small raster.
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

    /// Root-fix proof (Phase 4a content-digest key): a DIFFERENT raster of the SAME
    /// size registered under the SAME caller hash must NOT serve the first raster's
    /// cached facts — the content key differs, so it is a correct miss. This is exactly
    /// the same-size collision the `backs` self-heal could NOT catch (dimensions match);
    /// the digest key eliminates it at the root. Would go red under the old u64 keying
    /// (stale facts aliased, one cache entry).
    #[test]
    fn same_hash_different_content_does_not_alias() {
        let mut s = RenderSession::new();
        s.set_look(spectrum());
        s.register("x", 5, solid_source(30, 120, 200)); // 256², hash 5
        let mut d = ComposeDiagnostics::default();
        let _ = s.render("x", false, false, 256, &RenderOpts::default(), &mut d).unwrap();

        let b = plated_source(); // 256², DIFFERENT content, SAME hash 5
        s.register("x", 5, b.clone());
        let mut d2 = ComposeDiagnostics::default();
        let after = s.render("x", false, false, 256, &RenderOpts::default(), &mut d2).unwrap();
        assert_eq!(after.data, free_render(&b, 256), "same-hash different-content aliased stale facts");
        assert_eq!(s.cached_source_facts_count(), 2, "distinct content must be distinct cache entries");
    }

    /// The digest key also MERGES: the same raster under two different caller hashes
    /// shares one cache entry (facts are a pure function of the pixels, so this is
    /// byte-correct and strictly a reuse win).
    #[test]
    fn same_content_different_hash_shares_entry() {
        let mut s = RenderSession::new();
        s.set_look(spectrum());
        let src = plated_source();
        s.register("p", 100, src.clone());
        s.register("q", 200, src.clone()); // identical bytes, different caller hash
        let mut d = ComposeDiagnostics::default();
        let _ = s.render("p", false, false, 256, &RenderOpts::default(), &mut d).unwrap();
        let _ = s.render("q", false, false, 256, &RenderOpts::default(), &mut d).unwrap();
        assert_eq!(s.cached_source_facts_count(), 1, "identical content must share one entry regardless of caller hash");
    }
}
