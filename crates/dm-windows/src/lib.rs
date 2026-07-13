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
pub mod cmdline;
pub mod durable;
pub mod fingerprint_surface;
pub mod pathcheck;
// The calm (清爽) settings platform adapters. Cross-platform MODULE: the `translate`/`profile_facts`
// decision cores compile and are unit-tested on the Mac host; the `WinregBackend`/profile FFI shells
// inside are `cfg(windows)` and `[WINDOWS-VERIFY]` (Wave 2).
pub mod system_tweaks;
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
// Cross-platform MODULE (the extractor struct/COM body inside is `cfg(windows)`): the pure
// pixel/parse helpers (premul→straight, icon-location parse, %ENV% expand) are exercised on the
// Mac host; only the shell/GDI runtime is `[WINDOWS-VERIFY]` — same split as `watcher`.
mod source;
// Cross-platform MODULE (struct/COM cfg(windows) inside): the desktop-class predicate is
// exercised on the Mac host; live GetForegroundWindow/GetLastInputInfo are [WINDOWS-VERIFY].
mod activity;
#[cfg(windows)]
mod state_reader;
#[cfg(windows)]
pub mod topology;
#[cfg(windows)]
pub mod wallpaper;
// Cross-platform (notify-backed) — NOT `cfg(windows)`: the debounce + event-mapping core is
// exercised on the Mac host (B10), only the Windows-runtime desktop semantics are `[WINDOWS-VERIFY]`.
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
pub use activity::WindowsActivityMonitor;
#[cfg(windows)]
pub use shell::WindowsDesktopGeometry;
#[cfg(windows)]
pub use source::WindowsIconSourceExtractor;
#[cfg(windows)]
pub use state_reader::WindowsStateReader;
#[cfg(windows)]
pub use system_tweaks::{WindowsSystemProfileProbe, WinregBackend};
#[cfg(windows)]
pub use topology::WindowsMonitorTopology;
#[cfg(windows)]
pub use wallpaper::WindowsWallpaper;
