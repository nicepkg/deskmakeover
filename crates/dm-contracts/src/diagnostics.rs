//! Diagnostics DTOs. `DiagnosticsPing` is the smallest possible round-trip that
//! proves the whole binding pipeline end to end (Rust command → generated TS →
//! `invoke`) without depending on any platform surface.

use serde::{Deserialize, Serialize};
use specta::Type;

/// Reply to `diagnostics_ping` — echoes the caller's message and reports the
/// backend version, so the frontend can confirm it is talking to a live host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsPing {
    pub ok: bool,
    pub version: String,
    pub message: String,
}
