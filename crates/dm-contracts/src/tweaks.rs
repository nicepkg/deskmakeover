//! 清爽 (calm-Windows) settings DTOs — the Rust source for the calm bridge in `bridge/types.ts`
//! (BRIDGE_SCHEMA_VERSION 8). Field names and enum string literals mirror the frontend
//! `CalmBackend` port shapes (`src/bridge/mock-calm.ts`) EXACTLY, so the generated bindings drop
//! into `tauri.ts` and the store never learns whether it is talking to the mock or real Rust.
//!
//! THIN by design (the same D1 boundary as icons/wallpaper): Rust reports each row's honest probe
//! outcome and each apply/restore outcome; the frontend owns grouping, the hero phase, the
//! schematic, and every rendering concern. The host maps the richer `dm_operations` outcomes onto
//! these wire shapes.

use serde::{Deserialize, Serialize};
use specta::Type;

/// The probe state of one calm row, mirroring the TS `CalmProbeState` union. The `ownedByUs` /
/// `driftedFromUs` flags on [`CalmProbeRowDto`] refine `quiet`/`pushing` into the ledger-aware
/// frontend states (`verified` / `reopened`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum CalmProbeStateDto {
    Quiet,
    Pushing,
    Unsupported,
    Managed,
    NeedsReconfirm,
}

/// One row's probe result. `ownedByUs` = the ledger owns it and its value is intact (→ verified);
/// `driftedFromUs` = the ledger owns it but the value moved (→ reopened). Both false for a plain
/// unowned row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CalmProbeRowDto {
    pub id: String,
    pub state: CalmProbeStateDto,
    pub owned_by_us: bool,
    pub drifted_from_us: bool,
}

/// The outcome of applying one row, mirroring the TS `CalmApplyRow.outcome` union.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum CalmApplyOutcomeDto {
    Verified,
    SetAwaiting,
    Reverted,
    Skipped,
}

/// Why an apply skipped a row without writing it (only `Changed` today), mirroring the TS
/// `CalmApplyRow.reason`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum CalmSkipReasonDto {
    Changed,
}

/// One row's apply result. `reason` is present only when `outcome` is `skipped`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CalmApplyRowDto {
    pub id: String,
    pub outcome: CalmApplyOutcomeDto,
    pub reason: Option<CalmSkipReasonDto>,
}

/// The outcome of restoring one row, mirroring the TS `CalmRestoreRow.outcome` union.
/// `SkippedDrift` = the row was hand-edited since our write, so it is disowned, never clobbered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum CalmRestoreOutcomeDto {
    Restored,
    SkippedDrift,
}

/// One row's restore result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CalmRestoreRowDto {
    pub id: String,
    pub outcome: CalmRestoreOutcomeDto,
}

/// A guided row's return-probe answer, mirroring the TS `reProbeGuided` return
/// (`boolean | null`): `Some(true)` off, `Some(false)` still on, `None` unreadable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum CalmGuidedProbeDto {
    Off,
    StillOn,
    Unreadable,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_state_serializes_to_the_ts_union_literals() {
        assert_eq!(
            serde_json::to_string(&CalmProbeStateDto::NeedsReconfirm).unwrap(),
            "\"needsReconfirm\""
        );
        assert_eq!(
            serde_json::to_string(&CalmProbeStateDto::Quiet).unwrap(),
            "\"quiet\""
        );
    }

    #[test]
    fn apply_outcome_matches_the_frontend_literals() {
        assert_eq!(
            serde_json::to_string(&CalmApplyOutcomeDto::SetAwaiting).unwrap(),
            "\"setAwaiting\""
        );
    }

    #[test]
    fn a_skipped_apply_row_carries_its_reason() {
        let row = CalmApplyRowDto {
            id: "taskbar.search".into(),
            outcome: CalmApplyOutcomeDto::Skipped,
            reason: Some(CalmSkipReasonDto::Changed),
        };
        let json = serde_json::to_value(&row).unwrap();
        assert_eq!(json["outcome"], "skipped");
        assert_eq!(json["reason"], "changed");
        assert_eq!(json["id"], "taskbar.search");
    }

    #[test]
    fn probe_row_uses_camel_case_flag_names() {
        let row = CalmProbeRowDto {
            id: "taskbar.search".into(),
            state: CalmProbeStateDto::Quiet,
            owned_by_us: true,
            drifted_from_us: false,
        };
        let json = serde_json::to_value(&row).unwrap();
        assert_eq!(json["ownedByUs"], true);
        assert_eq!(json["driftedFromUs"], false);
    }
}
