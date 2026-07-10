//! Windows platform layer (ADR-0019): ALL windows-rs / COM / unsafe lives here —
//! STA actor, scan, layout, extract, .lnk/.url, desktop.ini, system icons,
//! wallpaper, watcher, Explorer refresh. Empty until M4; future code is
//! `#[cfg(windows)]`-gated so the workspace stays green on macOS.
