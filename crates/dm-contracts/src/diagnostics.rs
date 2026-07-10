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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> DiagnosticsPing {
        DiagnosticsPing { ok: true, version: "0.0.0".into(), message: "hello".into() }
    }

    #[test]
    fn round_trips_through_json() {
        let ping = sample();
        let json = serde_json::to_string(&ping).unwrap();
        let back: DiagnosticsPing = serde_json::from_str(&json).unwrap();
        assert_eq!(ping, back);
    }

    #[test]
    fn json_keys_are_the_ts_field_names() {
        let json = serde_json::to_string(&sample()).unwrap();
        assert!(json.contains("\"ok\":true"), "got {json}");
        assert!(json.contains("\"version\":\"0.0.0\""));
        assert!(json.contains("\"message\":\"hello\""));
    }

    #[test]
    fn tolerates_unknown_fields_from_a_newer_frontend() {
        // No `deny_unknown_fields` — an extra field from a newer TS build must not break decode.
        let ping: DiagnosticsPing =
            serde_json::from_str(r#"{"ok":false,"version":"9","message":"m","extra":123}"#).unwrap();
        assert!(!ping.ok);
        assert_eq!(ping.version, "9");
    }

    #[test]
    fn message_may_be_empty_or_unicode() {
        for msg in ["", "こんにちは 🌸", "line1\nline2"] {
            let ping = DiagnosticsPing { ok: true, version: "1".into(), message: msg.into() };
            let back: DiagnosticsPing =
                serde_json::from_str(&serde_json::to_string(&ping).unwrap()).unwrap();
            assert_eq!(back.message, msg);
        }
    }
}
