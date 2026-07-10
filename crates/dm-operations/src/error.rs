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

    #[error("i/o failure: {0}")]
    Io(String),

    #[error("journal error: {0}")]
    Journal(String),

    #[error("serialization error: {0}")]
    Serde(String),

    /// The active ledger file exists but could not be parsed. This is fail-closed on purpose
    /// (oracle `DesktopBakeService.LoadState` codex B4): a corrupt ledger must NEVER read as
    /// "nothing applied", which would strand the only path back to the user's original.
    #[error("active ledger is present but unreadable; refusing to treat as empty")]
    CorruptLedger,

    /// A platform port failed while driving a transaction.
    #[error("platform port: {0}")]
    Port(#[from] dm_domain::PortError),
}

impl From<std::io::Error> for OperationError {
    fn from(e: std::io::Error) -> Self {
        OperationError::Io(e.to_string())
    }
}

impl From<serde_json::Error> for OperationError {
    fn from(e: serde_json::Error) -> Self {
        OperationError::Serde(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, OperationError>;
