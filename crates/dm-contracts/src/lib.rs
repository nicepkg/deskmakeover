//! Contracts (ADR-0019): serde DTOs and generated TypeScript bindings
//! (tauri-specta) — the single contract source; hand-mirrored schemas are banned.
//!
//! M2 covers the first slice only: settings get/set + a diagnostics ping. Every
//! DTO derives `specta::Type`, so `apps/desktop/src-tauri` exports byte-identical
//! TypeScript into `src/bridge/generated.ts` (drift is a CI failure).
//! The rest of bridge schema 4 keeps living in `bridge/types.ts` until later
//! phases migrate it here.

mod diagnostics;
mod settings;

pub use diagnostics::DiagnosticsPing;
pub use settings::{Language, SettingsDto, SettingsPatch, Theme};
