//! Copyable reference boundary for DeskMakeover Windows system settings.
//!
//! Pure models and ports compile on every host. Concrete Windows adapters are target-gated so
//! macOS can run unit tests while an explicit MSVC target checks the real windows-rs signatures.

mod model;
mod ports;
mod reference_bridge;

pub use model::*;
pub use ports::*;
pub use reference_bridge::*;

#[cfg(windows)]
mod windows_backend;

#[cfg(windows)]
pub use windows_backend::{WinRegistryBackend, WindowsRefreshBackend, WindowsSystemProfileProbe};
