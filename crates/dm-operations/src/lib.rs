//! Operations (ADR-0019): durable transaction ledger, snapshots, and
//! compare-and-swap restore — incremental owned-field apply, no restore-first port.
//!
//! M2 lands the first persistent store: [`SettingsStore`], a rusqlite-backed,
//! version-migrated home for the settings row. The ledger/snapshot machinery
//! arrives with the M3 vertical slice; this module already owns the schema
//! migration pattern the rest of persistence will reuse.

mod error;
mod settings_store;

pub use error::{OperationError, Result};
pub use settings_store::SettingsStore;
