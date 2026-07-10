//! Typed errors for the operations layer. Kept small and `Send + Sync` so Tauri
//! commands can surface a stable string to the frontend without leaking rusqlite
//! internals into the contract.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum OperationError {
    #[error("settings store i/o: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("unsupported settings schema version {found}; this build expects {expected}")]
    SchemaTooNew { found: u32, expected: u32 },

    #[error("settings store data corrupt: {0}")]
    Corrupt(String),
}

pub type Result<T> = std::result::Result<T, OperationError>;
