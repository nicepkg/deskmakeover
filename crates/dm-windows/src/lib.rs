//! `dm-windows` — the Windows platform adapters implementing the `dm-domain` ports.
//!
//! Everything that touches COM / the registry / the shell lives behind `#[cfg(windows)]` and is
//! **blind-written on Mac**: kept type-checking against `windows-rs` via
//! `cargo check -p dm-windows --target x86_64-pc-windows-msvc`, with all runtime verification
//! deferred to the owner's Windows box (every such item is tagged `[WINDOWS-VERIFY]`).
//!
//! COM discipline (ADR-0019 Amendment 1): all apartment-threaded COM runs on a single dedicated
//! STA thread ([`com::StaExecutor`]); COM interface pointers are created and released inside that
//! thread and never cross a thread boundary or an `.await`. Public methods send owned data in and
//! receive owned data back.
//!
//! Pure logic that needs no platform API (item classification, icon-location parsing) lives in
//! [`classify`], which compiles and is unit-tested on the host.

pub mod classify;
pub mod fingerprint_surface;
pub mod textfmt;

#[cfg(windows)]
pub mod apply;
#[cfg(windows)]
pub mod com;
#[cfg(windows)]
mod overlay;
#[cfg(windows)]
mod refresh;
#[cfg(windows)]
pub mod shell;
#[cfg(windows)]
mod state_reader;
#[cfg(windows)]
pub mod wallpaper;
#[cfg(windows)]
pub mod watcher;

#[cfg(windows)]
pub use apply::WindowsIconApplier;
#[cfg(windows)]
pub use com::StaExecutor;
#[cfg(windows)]
pub use overlay::WindowsOverlayControl;
#[cfg(windows)]
pub use refresh::WindowsExplorerRefresher;
#[cfg(windows)]
pub use shell::WindowsScanner;
#[cfg(windows)]
pub use state_reader::WindowsStateReader;
#[cfg(windows)]
pub use wallpaper::WindowsWallpaper;
