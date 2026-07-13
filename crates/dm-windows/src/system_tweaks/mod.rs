//! The Windows platform adapters for the calm (清爽) settings decision core: the raw registry
//! [`WinregBackend`] and the [`WindowsSystemProfileProbe`], implementing the
//! `dm_domain::system_tweaks` ports the `dm-operations` `TweakDriver` depends on.
//!
//! Wave 2 (`[WINDOWS-VERIFY]`): blind-written on Mac and kept compiling against real `windows-rs`
//! via `cargo check -p dm-windows --target x86_64-pc-windows-msvc`, with runtime verification
//! deferred to the Wave 3 certification lab. The [`translate`] and [`profile_facts`] cores are
//! platform-agnostic and unit-tested on the Mac host; only [`backend`] and [`profile`] are
//! `cfg(windows)` FFI shells, so every DECISION is verified without a Windows box and only the raw
//! syscalls carry the `[WINDOWS-VERIFY]` risk.

pub mod profile_facts;
pub mod translate;

#[cfg(windows)]
mod backend;
#[cfg(windows)]
mod profile;

#[cfg(windows)]
pub use backend::WinregBackend;
#[cfg(windows)]
pub use profile::WindowsSystemProfileProbe;
