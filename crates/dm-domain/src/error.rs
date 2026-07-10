//! Typed errors crossing the port boundary between the pure operations layer and the
//! platform (COM/registry/filesystem) layer.

use thiserror::Error;

/// A failure reported by a platform port. Variants stay coarse on purpose — the operations
/// layer branches on *kind of failure* (is it a conflict? is the item gone?), not on Win32
/// error codes.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PortError {
    /// The target item no longer exists (deleted between scan and apply).
    #[error("item no longer exists: {0}")]
    NotFound(String),

    /// The generated asset the apply needs is missing.
    #[error("generated asset missing: {0}")]
    AssetMissing(String),

    /// The item's current state does not match what the caller expected — an external
    /// modification (spec 07 §5 conflict). Never overwritten silently.
    #[error("item changed after it was observed: {0}")]
    Conflict(String),

    /// An underlying I/O / filesystem failure.
    #[error("i/o failure: {0}")]
    Io(String),

    /// An underlying COM / shell failure (an `HRESULT`, a released view, an MTA thread).
    #[error("shell/com failure: {0}")]
    Com(String),

    /// The operation is not supported for this item kind.
    #[error("unsupported for this item: {0}")]
    Unsupported(String),
}

/// Convenience alias for port results.
pub type PortResult<T> = Result<T, PortError>;
