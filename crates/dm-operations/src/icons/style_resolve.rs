//! Native style resolution — the Rust port of the frontend's resolve ladder, for render paths
//! with no webview (the resident reconciler + version switching, spec 07 §9/§15).
//!
//! Faithful to the TS truth (`src/lib/kind-policy.ts`, `src/lib/type-config.ts`,
//! `src/stores/icons.ts effectiveTileConfig`), minus per-icon overrides — a background item is by
//! definition one the user has never styled individually, and per-icon overrides deliberately do
//! not persist inside an appearance recipe (spec 07 §8 non-scope). Ladder (spec 06 §6):
//! type-override patch over the base config, then the opt-in uniform shortcut shape, then the
//! kind-policy participation check.
//!
//! `IconStyle` stays the single validated JSON truth — this module PARSES it (serde, deny nothing:
//! unknown fields are the frontend's future) and converts the resolved `ConfigDto` shape into the
//! `dm_icon_core::config::Config` the native `RenderSession` consumes. Enum spellings follow the
//! TS unions verbatim; an unknown spelling is an error, never a silent default (a mis-parsed axis
//! changes pixels).

use dm_contracts::IconStyle;
use dm_icon_core::config::{
    Band, Config as CoreConfig, Distinction, FilterStyle, IconShape, MarkStyle, MonoStyle,
    PlateFallback, Subject,
};
use dm_domain::ItemKind;
use serde::Deserialize;

use crate::error::{OperationError, Result};

/// The three governed buckets (`IconKindBucket`). The former System bucket merged into App
/// (owner 2026-07-16, TS `kind-policy.ts`): Recycle Bin / This PC read as programs to the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KindBucket {
    App,
    Folder,
    File,
}

/// The bucket an item kind belongs to; `None` = ungoverned (Unsupported) — governed by
/// `styleable`, not the policy (TS `kindBucket`).
pub fn bucket_of(kind: ItemKind) -> Option<KindBucket> {
    match kind {
        ItemKind::Shortcut
        | ItemKind::UrlShortcut
        | ItemKind::AppxShortcut
        | ItemKind::RecycleBin
        | ItemKind::System => Some(KindBucket::App),
        ItemKind::Folder => Some(KindBucket::Folder),
        ItemKind::RegularFile => Some(KindBucket::File),
        ItemKind::Unsupported => None,
    }
}

/// `ConfigDto` as persisted inside an `IconStyle` recipe (camelCase JSON).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigDtoJson {
    pub shape: String,
    pub subject: String,
    pub tint: String,
    pub mono_style: String,
    pub plate_band: String,
    #[serde(default)]
    pub shortcut_shape: Option<String>,
    pub distinction: String,
    pub mark_style: String,
    #[serde(default)]
    pub mark_color: Option<String>,
    pub filter: String,
    #[serde(default)]
    pub plate_color: Option<String>,
    pub plate_fallback: String,
}

/// A type override's source (`TypeOverrideEntry.source`, TS union): `follow` = takes the base
/// config, `custom` = applies its patch. Modeled as an enum (codex m7a-🟠6): an unknown spelling
/// like `"custmo"` MUST error, never silently fall through to `follow` and change the pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OverrideSource {
    Follow,
    Custom,
}

/// One bucket's override entry (`TypeOverrideEntry`): only `source == custom` with a patch
/// participates in the ladder. A present entry with a garbage `source` is an ERROR at parse time.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeOverrideJson {
    #[serde(default)]
    pub source: Option<OverrideSource>,
    #[serde(default)]
    pub patch: Option<TypePatchJson>,
}

/// The envelope of keys a type may override (spec 06 §6.5 — filter and Original-mode are
/// global-only by law, so they are absent here BY DESIGN).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypePatchJson {
    #[serde(default)]
    pub shape: Option<String>,
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub tint: Option<String>,
    #[serde(default)]
    pub plate_band: Option<String>,
    #[serde(default)]
    pub mono_style: Option<String>,
    // `plateColor: null` inside a patch is MEANINGFUL (随图标) and distinct from absent — a
    // double Option keeps the tri-state.
    #[serde(default, deserialize_with = "double_option")]
    pub plate_color: Option<Option<String>>,
    #[serde(default)]
    pub plate_fallback: Option<String>,
}

fn double_option<'de, D>(d: D) -> std::result::Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::<String>::deserialize(d)?))
}

/// Per-bucket participation. Every bucket participates BY DEFAULT (the app beautifies everything
/// until the user opts a bucket out — TS `DEFAULT_KIND_POLICY`). The hand-written `Default`
/// (all-true) is load-bearing (codex m7a-🟠6): the derived all-FALSE default would make a recipe
/// with a MISSING `kindPolicy` mean the OPPOSITE of `{}` — silently opting every bucket OUT.
/// A legacy `"System"` key in persisted JSON is silently ignored (serde default struct
/// tolerance) — old recipes stay loadable after the System→App merge (owner 2026-07-16).
#[derive(Debug, Clone, Deserialize)]
pub struct KindPolicyJson {
    #[serde(default = "yes", rename = "App")]
    pub app: bool,
    #[serde(default = "yes", rename = "Folder")]
    pub folder: bool,
    #[serde(default = "yes", rename = "File")]
    pub file: bool,
}

impl Default for KindPolicyJson {
    fn default() -> Self {
        Self { app: true, folder: true, file: true }
    }
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeOverridesJson {
    #[serde(default, rename = "App")]
    pub app: Option<TypeOverrideJson>,
    #[serde(default, rename = "Folder")]
    pub folder: Option<TypeOverrideJson>,
    #[serde(default, rename = "File")]
    pub file: Option<TypeOverrideJson>,
}

/// The parsed appearance recipe: the three global knobs of an `IconStyle`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleRecipe {
    pub config: ConfigDtoJson,
    #[serde(default)]
    pub kind_policy: KindPolicyJson,
    #[serde(default)]
    pub type_overrides: TypeOverridesJson,
}

impl StyleRecipe {
    /// Parses a validated `IconStyle` into the typed recipe.
    pub fn parse(style: &IconStyle) -> Result<Self> {
        serde_json::from_value(style.as_value().clone())
            .map_err(|e| OperationError::InvalidPayload(format!("style recipe parse: {e}")))
    }

    fn override_for(&self, bucket: KindBucket) -> Option<&TypeOverrideJson> {
        match bucket {
            KindBucket::App => self.type_overrides.app.as_ref(),
            KindBucket::Folder => self.type_overrides.folder.as_ref(),
            KindBucket::File => self.type_overrides.file.as_ref(),
        }
    }

    fn participates(&self, kind: ItemKind) -> bool {
        match bucket_of(kind) {
            None => true,
            Some(KindBucket::App) => self.kind_policy.app,
            Some(KindBucket::Folder) => self.kind_policy.folder,
            Some(KindBucket::File) => self.kind_policy.file,
        }
    }

    /// True when the bucket's custom patch asserts its own SHAPE (TS
    /// `typeAssertsShape`): the more specific assertion, so the uniform
    /// shortcut shape yields to it (owner 2026-07-16).
    fn type_asserts_shape(&self, bucket: Option<KindBucket>) -> bool {
        let Some(bucket) = bucket else { return false };
        self.override_for(bucket).is_some_and(|entry| {
            entry.source == Some(OverrideSource::Custom)
                && entry.patch.as_ref().is_some_and(|p| p.shape.is_some())
        })
    }

    /// The config an item of `kind` renders with under this recipe, or `None` when the item does
    /// not participate (kind-policy opt-out → keep original). Mirrors `effectiveTileConfig` for
    /// items with no per-icon override (the background item's definition).
    pub fn effective_config(&self, kind: ItemKind, is_shortcut: bool) -> Result<Option<CoreConfig>> {
        if !self.participates(kind) {
            return Ok(None);
        }
        let mut cfg = self.config.clone();
        if let Some(bucket) = bucket_of(kind) {
            if let Some(entry) = self.override_for(bucket) {
                if entry.source == Some(OverrideSource::Custom) {
                    if let Some(patch) = &entry.patch {
                        apply_patch(&mut cfg, patch);
                    }
                }
            }
        }
        // Shape precedence (owner 2026-07-16, TS `effectiveTileConfig` in lockstep):
        // type patch shape > uniform shortcut shape > global shape. The opt-in
        // shortcut uniform applies only when the item's bucket didn't assert its
        // own shape — a folder/file shortcut keeps its type's shape and still
        // renders the shortcut mark (that layer keys on `is_shortcut` alone).
        if is_shortcut && !self.type_asserts_shape(bucket_of(kind)) {
            if let Some(shape) = cfg.shortcut_shape.clone() {
                cfg.shape = shape;
            }
        }
        to_core_config(&cfg).map(Some)
    }
}

fn apply_patch(cfg: &mut ConfigDtoJson, patch: &TypePatchJson) {
    if let Some(v) = &patch.shape {
        cfg.shape = v.clone();
    }
    if let Some(v) = &patch.subject {
        cfg.subject = v.clone();
    }
    if let Some(v) = &patch.tint {
        cfg.tint = v.clone();
    }
    if let Some(v) = &patch.plate_band {
        cfg.plate_band = v.clone();
    }
    if let Some(v) = &patch.mono_style {
        cfg.mono_style = v.clone();
    }
    if let Some(v) = &patch.plate_color {
        cfg.plate_color = v.clone();
    }
    if let Some(v) = &patch.plate_fallback {
        cfg.plate_fallback = v.clone();
    }
}

/// `hexToInt`: `#RRGGBB` (case-insensitive, `#` optional) → packed `0xRRGGBB`.
pub fn hex_to_int(hex: &str) -> Result<u32> {
    let h = hex.trim().trim_start_matches('#');
    if h.len() != 6 {
        return Err(OperationError::InvalidPayload(format!("colour {hex:?} is not #RRGGBB")));
    }
    u32::from_str_radix(h, 16).map_err(|e| OperationError::InvalidPayload(format!("colour {hex:?}: {e}")))
}

fn parse_shape(s: &str) -> Result<IconShape> {
    Ok(match s {
        "Apple" => IconShape::Apple,
        "Circle" => IconShape::Circle,
        "Samsung" => IconShape::Samsung,
        "None" => IconShape::None,
        "Bookmark" => IconShape::Bookmark,
        "Lemon" => IconShape::Lemon,
        "Tile" => IconShape::Tile,
        "Teardrop" => IconShape::Teardrop,
        "Diamond" => IconShape::Diamond,
        "Flower" => IconShape::Flower,
        "Pebble" => IconShape::Pebble,
        "Folder" => IconShape::Folder,
        "File" => IconShape::File,
        other => return Err(OperationError::InvalidPayload(format!("unknown shape {other:?}"))),
    })
}

/// Converts the resolved `ConfigDto` shape into the core render `Config` — the same mapping the
/// worker's 24-byte ABI performs (`dm-icon-wasm/src/abi.rs`), from the JSON string spellings.
pub fn to_core_config(cfg: &ConfigDtoJson) -> Result<CoreConfig> {
    let subject = match cfg.subject.as_str() {
        "Original" => Subject::Original,
        "BlackWhite" => Subject::BlackWhite,
        "Mono" => Subject::Mono,
        other => return Err(OperationError::InvalidPayload(format!("unknown subject {other:?}"))),
    };
    let mono_style = match cfg.mono_style.as_str() {
        "Tonal" => MonoStyle::Tonal,
        "Flat" => MonoStyle::Flat,
        other => return Err(OperationError::InvalidPayload(format!("unknown monoStyle {other:?}"))),
    };
    let plate_band = match cfg.plate_band.as_str() {
        "Vivid" => Band::Vivid,
        "Quiet" => Band::Quiet,
        other => return Err(OperationError::InvalidPayload(format!("unknown plateBand {other:?}"))),
    };
    let distinction = match cfg.distinction.as_str() {
        "Mark" => Distinction::Mark,
        "Keep" => Distinction::Keep,
        "None" => Distinction::None,
        other => return Err(OperationError::InvalidPayload(format!("unknown distinction {other:?}"))),
    };
    let mark_style = match cfg.mark_style.as_str() {
        "Glass" => MarkStyle::Glass,
        "Shadow" => MarkStyle::Shadow,
        "Halo" => MarkStyle::Halo,
        "Satin" => MarkStyle::Satin,
        "Arc" => MarkStyle::Arc,
        "Fold" => MarkStyle::Fold,
        "Ring" => MarkStyle::Ring,
        "Comet" => MarkStyle::Comet,
        other => return Err(OperationError::InvalidPayload(format!("unknown markStyle {other:?}"))),
    };
    let filter = match cfg.filter.as_str() {
        "None" => FilterStyle::None,
        "Gloss" => FilterStyle::Gloss,
        "Glass" => FilterStyle::Glass,
        "Pixel" => FilterStyle::Pixel,
        "Sticker" => FilterStyle::Sticker,
        other => return Err(OperationError::InvalidPayload(format!("unknown filter {other:?}"))),
    };
    let plate_fallback = match cfg.plate_fallback.as_str() {
        "derived" => PlateFallback::Derived,
        "white" => PlateFallback::White,
        other => return Err(OperationError::InvalidPayload(format!("unknown plateFallback {other:?}"))),
    };
    Ok(CoreConfig {
        shape: parse_shape(&cfg.shape)?,
        subject,
        tint: hex_to_int(&cfg.tint)?,
        mono_style,
        plate_band,
        shortcut_shape: match &cfg.shortcut_shape {
            Some(s) => Some(parse_shape(s)?),
            None => None,
        },
        distinction,
        mark_style,
        mark_color: match &cfg.mark_color {
            Some(c) => Some(hex_to_int(c)?),
            None => None,
        },
        filter,
        plate_color: match &cfg.plate_color {
            Some(c) => Some(hex_to_int(c)?),
            None => None,
        },
        plate_fallback,
    })
}

/// Brand accents rotated onto COLOURLESS App-bucket sources (owner special case 2026-07-10,
/// ADR-0017; TS `appAccentSeed`) — deterministic per id, coral + teal ONLY.
pub const APP_ACCENT_SEEDS: [u32; 2] = [0x00FF_6F5E, 0x003F_B6A8];

pub fn app_accent_seed(id: &str) -> u32 {
    let mut h: u32 = 0;
    for u in id.encode_utf16() {
        h = h.wrapping_mul(31).wrapping_add(u as u32);
    }
    APP_ACCENT_SEEDS[(h % 2) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn style(v: serde_json::Value) -> IconStyle {
        IconStyle::from_value(v).expect("valid style")
    }

    fn base_config() -> serde_json::Value {
        json!({
            "shape": "Circle", "subject": "Original", "tint": "#FF6F5E",
            "monoStyle": "Tonal", "plateBand": "Vivid", "shortcutShape": null,
            "distinction": "None", "markStyle": "Glass", "markColor": null,
            "size": "Mid", "filter": "None", "plateColor": null, "plateFallback": "derived"
        })
    }

    #[test]
    fn parses_and_resolves_the_base_config_for_a_participating_kind() {
        let s = style(json!({ "config": base_config(), "kindPolicy": {}, "typeOverrides": {} }));
        let recipe = StyleRecipe::parse(&s).unwrap();
        let cfg = recipe.effective_config(ItemKind::Shortcut, true).unwrap().unwrap();
        assert_eq!(cfg.shape, IconShape::Circle);
        assert_eq!(cfg.subject, Subject::Original);
        assert_eq!(cfg.tint, 0xFF6F5E);
        assert_eq!(cfg.plate_color, None);
        assert_eq!(cfg.plate_fallback, PlateFallback::Derived);
    }

    #[test]
    fn kind_policy_opt_out_yields_none_and_unsupported_bypasses_the_policy() {
        // The legacy "System" key (pre-merge recipes) must stay tolerated — ignored, never an error.
        let s = style(json!({
            "config": base_config(),
            "kindPolicy": { "App": true, "Folder": false, "File": true, "System": false },
            "typeOverrides": {}
        }));
        let recipe = StyleRecipe::parse(&s).unwrap();
        assert!(recipe.effective_config(ItemKind::Folder, false).unwrap().is_none());
        assert!(recipe.effective_config(ItemKind::Shortcut, true).unwrap().is_some());
        // System virtual items ride the App switch after the merge (owner 2026-07-16) —
        // the legacy System:false above has no effect.
        assert!(recipe.effective_config(ItemKind::RecycleBin, false).unwrap().is_some());
        // Bucketless kinds pass the policy (governed by styleable upstream).
        assert!(recipe.effective_config(ItemKind::Unsupported, false).unwrap().is_some());
    }

    #[test]
    fn a_custom_type_patch_overrides_only_its_keys_and_a_follower_takes_base() {
        let s = style(json!({
            "config": base_config(),
            "kindPolicy": {},
            "typeOverrides": {
                "Folder": { "source": "custom", "patch": { "shape": "Folder", "tint": "#112233" } },
                "File": { "source": "follow" }
            }
        }));
        let recipe = StyleRecipe::parse(&s).unwrap();
        let folder = recipe.effective_config(ItemKind::Folder, false).unwrap().unwrap();
        assert_eq!(folder.shape, IconShape::Folder);
        assert_eq!(folder.tint, 0x112233);
        assert_eq!(folder.subject, Subject::Original, "unpatched keys keep base");
        let file = recipe.effective_config(ItemKind::RegularFile, false).unwrap().unwrap();
        assert_eq!(file.shape, IconShape::Circle, "a follower takes base");
    }

    #[test]
    fn the_uniform_shortcut_shape_rides_on_top_of_the_type_ladder() {
        let mut cfg = base_config();
        cfg["shortcutShape"] = json!("Diamond");
        let s = style(json!({ "config": cfg, "kindPolicy": {}, "typeOverrides": {} }));
        let recipe = StyleRecipe::parse(&s).unwrap();
        let shortcut = recipe.effective_config(ItemKind::Shortcut, true).unwrap().unwrap();
        assert_eq!(shortcut.shape, IconShape::Diamond, "shortcuts adopt the uniform shape");
        let folder = recipe.effective_config(ItemKind::Folder, false).unwrap().unwrap();
        assert_eq!(folder.shape, IconShape::Circle, "non-shortcuts keep the ladder shape");
    }

    #[test]
    fn a_type_patch_shape_beats_the_uniform_shortcut_shape() {
        // Shape precedence (owner 2026-07-16): type patch shape > uniform shortcut
        // shape > global shape — one rule for every bucket. Kind carries TARGET
        // semantics: a folder shortcut arrives as ItemKind::Folder + is_shortcut.
        let mut cfg = base_config();
        cfg["shortcutShape"] = json!("Diamond");
        let s = style(json!({
            "config": cfg,
            "kindPolicy": {},
            "typeOverrides": {
                "Folder": { "source": "custom", "patch": { "shape": "Folder" } },
                "App": { "source": "custom", "patch": { "shape": "Tile" } },
                "File": { "source": "custom", "patch": { "tint": "#3FB6A8" } }
            }
        }));
        let recipe = StyleRecipe::parse(&s).unwrap();
        // A folder shortcut keeps the Folder type's shape, not the uniform.
        let folder_lnk = recipe.effective_config(ItemKind::Folder, true).unwrap().unwrap();
        assert_eq!(folder_lnk.shape, IconShape::Folder, "type shape wins over the uniform");
        // The App bucket obeys the same precedence (owner: one rule everywhere).
        let app_lnk = recipe.effective_config(ItemKind::Shortcut, true).unwrap().unwrap();
        assert_eq!(app_lnk.shape, IconShape::Tile, "App type shape wins too");
        // A patch WITHOUT a shape still yields to the uniform.
        let file_lnk = recipe.effective_config(ItemKind::RegularFile, true).unwrap().unwrap();
        assert_eq!(file_lnk.shape, IconShape::Diamond, "shape-less patch yields to the uniform");
        assert_eq!(file_lnk.tint, 0x3FB6A8, "the rest of the patch still applies");
        // Non-shortcut folders never take the uniform.
        let folder = recipe.effective_config(ItemKind::Folder, false).unwrap().unwrap();
        assert_eq!(folder.shape, IconShape::Folder);
    }

    #[test]
    fn unknown_enum_spellings_error_instead_of_defaulting() {
        let mut cfg = base_config();
        cfg["subject"] = json!("Technicolor");
        let s = style(json!({ "config": cfg, "kindPolicy": {}, "typeOverrides": {} }));
        let recipe = StyleRecipe::parse(&s).unwrap();
        assert!(recipe.effective_config(ItemKind::Shortcut, true).is_err());
    }

    #[test]
    fn the_kind_policy_default_participates_by_default_not_the_opposite() {
        // codex m7a-🟠6: the default must be all-TRUE (participate). A derived all-false default
        // would make a recipe whose `kindPolicy` deserializes via `#[serde(default)]` opt every
        // bucket OUT — the exact opposite of `{}`.
        let d = KindPolicyJson::default();
        assert!(d.app && d.folder && d.file, "every bucket participates by default");
        // And a StyleRecipe parsed from JSON with NO kindPolicy field takes that all-true default
        // (serde-level, below IconStyle's envelope validation).
        let recipe: StyleRecipe = serde_json::from_value(json!({
            "config": base_config(), "typeOverrides": {}
        }))
        .unwrap();
        assert!(recipe.effective_config(ItemKind::Folder, false).unwrap().is_some());
    }

    #[test]
    fn a_garbage_override_source_errors_instead_of_silently_following() {
        // codex m7a-🟠6: `"custmo"` must fail parse, never be treated as a follower (silently
        // dropping a custom patch and writing the base style).
        let s = IconStyle::from_value(json!({
            "config": base_config(),
            "kindPolicy": {},
            "typeOverrides": { "Folder": { "source": "custmo", "patch": { "shape": "Folder" } } }
        }))
        .unwrap();
        assert!(StyleRecipe::parse(&s).is_err(), "unknown source spelling is a parse error");
    }

    #[test]
    fn hex_colours_parse_and_reject_junk() {
        assert_eq!(hex_to_int("#FF6F5E").unwrap(), 0xFF6F5E);
        assert_eq!(hex_to_int("3fb6a8").unwrap(), 0x3FB6A8);
        assert!(hex_to_int("#12345").is_err());
        assert!(hex_to_int("red").is_err());
    }

    #[test]
    fn app_accent_seed_matches_the_ts_hash() {
        // TS: h = (h*31 + charCodeAt) >>> 0; seeds ['#FF6F5E', '#3FB6A8'].
        // "a" → 97 → odd → teal; "" → 0 → coral. Deterministic across runs.
        assert_eq!(app_accent_seed(""), APP_ACCENT_SEEDS[0]);
        assert_eq!(app_accent_seed("a"), APP_ACCENT_SEEDS[1]);
        assert_eq!(app_accent_seed("edge"), app_accent_seed("edge"));
    }
}
