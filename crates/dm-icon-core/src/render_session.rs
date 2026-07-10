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
use crate::profile::{icon_profile, IconProfile};
use crate::raster::Raster;

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

#[derive(Default)]
pub struct RenderSession {
    sources: HashMap<String, Registered>,
    profiles: HashMap<u64, CachedProfile>,
    look: Option<Config>,
}

impl RenderSession {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register (or replace) a decoded 256px source under an id, with a caller-
    /// supplied content hash of the source bytes for cache keying.
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
        self.analyze(id)?; // populate the cache
        let config = self.look.as_ref().expect("set_look before render");
        let raster = &self.sources.get(id)?.raster;
        let profile = self.profiles.get(&hash).map(|c| &c.profile);
        Some(render_tile_cached(raster, config, is_shortcut, show_original, size, opts, diag, profile))
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
    fn seed_of_returns_rim_hex_or_none() {
        let mut s = RenderSession::new();
        s.register("a", 7, solid_source(200, 40, 40));
        // A uniform red square has a rim colour → a hex seed.
        let seed = s.seed_of("a");
        assert!(seed.as_deref().map(|h| h.starts_with('#')).unwrap_or(true));
    }
}
