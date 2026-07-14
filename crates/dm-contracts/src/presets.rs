use serde::{Deserialize, Serialize};
use specta::Type;

// Preset packages (.dmpreset) + the user preset library — spec 09. The Rust
// side owns STRUCTURE and SECURITY (bounded unzip, string caps, PNG sniffing,
// atomic library writes); payload SEMANTICS (enum whitelists, clamping) belong
// to the ONE TS validator (`lib/icon-look.normalizeIconLook`, spec 09 §1) —
// `payload_json` therefore rides as an opaque, size-capped string here, and the
// import flow is read (Rust, pure) → validate (TS) → preview → save (Rust).

/// Shareable metadata for one preset entry (caps enforced on read AND save:
/// name ≤80 chars · author ≤80 · description ≤500; control chars stripped).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PresetMetaDto {
    pub name: String,
    pub author: Option<String>,
    pub description: Option<String>,
    /// ISO-8601 UTC, informational only.
    pub created_at: Option<String>,
}

/// One library entry (also the save/export input shape — library format ==
/// package format, spec 09 §1). `payload_json` is the serialized
/// IconLookPayload; thumbnails ride the `dmpreset://<id>` protocol for library
/// entries and inline base64 on package reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PresetEntryDto {
    pub id: String,
    /// "icon" now; "wallpaper" reserved (spec 09 §2).
    pub preset_type: String,
    pub schema_version: u32,
    pub meta: PresetMetaDto,
    pub payload_json: String,
    pub has_thumb: bool,
}

/// One entry as read out of a package: either a structurally valid candidate
/// (plus its sniffed PNG thumb, if any) or a per-entry failure reason — partial
/// success is first-class (spec 09 §5), one bad entry never sinks the pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PresetReadEntryDto {
    pub entry: Option<PresetEntryDto>,
    /// Bounded, sniffed PNG (base64) for the pre-import preview; never trusted
    /// as proof of the recipe (the app re-renders the authoritative preview).
    pub thumb_png_base64: Option<String>,
    /// Human-readable reason when `entry` is null (i18n happens in the web).
    pub error: Option<String>,
}

/// The result of reading a `.dmpreset` file (pure read — nothing touches the
/// library until `presets_save`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PresetPackageReadDto {
    /// Container format accepted (`dmpreset/1`). False = hard fail-closed
    /// (newer major or not a dmpreset) — `entries` is empty and `error` says why.
    pub format_ok: bool,
    pub entries: Vec<PresetReadEntryDto>,
    pub error: Option<String>,
}

/// Input for `presets_save` / one export entry: the entry body plus an optional
/// inline PNG thumb (base64, bounded, re-encoded by the webview renderer).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PresetSaveDto {
    /// Caller-supplied stable id (webview crypto.randomUUID); `[A-Za-z0-9-]{8,64}`.
    pub id: String,
    pub preset_type: String,
    pub schema_version: u32,
    pub meta: PresetMetaDto,
    pub payload_json: String,
    pub thumb_png_base64: Option<String>,
}
