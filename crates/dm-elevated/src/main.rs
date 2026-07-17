//! `dm-elevated` — the single privileged DeskMakeover binary (ADR-0019 Amendment 1, ADR-0021).
//!
//! It is a `requireAdministrator` helper whose ENTIRE job is the global shortcut-overlay verb
//! pair. There are no arbitrary commands and no scripting; the fixed verb set is parsed by
//! [`args`], the overlay ICO is validated by [`guards`], and the registry work lives in
//! [`overlay`]. Exit codes: 0 success, 2 an unknown/rejected verb, 3 a generic operation failure.
//! The desktop-items verbs additionally CLASSIFY the failure into a code the unelevated launcher
//! maps back to a human reason (`runas` has no stderr pipe, and writing a report file would be a
//! caller-controlled elevated write — codex 2026-07-17 P1): 10 = a target changed since the scan
//! (rescan + retry), 11 = access denied, 12 = a validation / unsupported-input rejection.
//!
//! The `requireAdministrator` elevation is applied at packaging (M8) via an external
//! `dm-elevated.exe.manifest`, so this crate has no resource-compiling build script and
//! cross-checks cleanly for `x86_64-pc-windows-msvc` on the host.

mod args;
mod desktop_items;
mod guards;
mod manifest;
mod overlay;
mod secure_dir;
mod session;

use std::process::ExitCode;

fn main() -> ExitCode {
    // args_os(), not args(): a non-Unicode argument must be REJECTED (exit 2), never panic the
    // privileged helper before parsing can refuse it (audit F6).
    let argv: Vec<String> = match std::env::args_os()
        .skip(1)
        .map(|a| a.into_string())
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(v) => v,
        Err(_) => {
            eprintln!("Unsupported helper operation: a non-Unicode argument was supplied");
            return ExitCode::from(2);
        }
    };
    match args::parse(&argv) {
        args::Command::None => {
            println!("DeskMakeover elevated helper. No operation requested.");
            ExitCode::SUCCESS
        }
        args::Command::Version => {
            println!("dm-elevated {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        args::Command::ApplyOverlay { style, file } => {
            finish(overlay::apply(style, file.as_deref()))
        }
        args::Command::RestoreOverlay => finish(overlay::restore()),
        args::Command::ApplyDesktopItems { manifest } => {
            finish_desktop_items(desktop_items::run_apply_file(&manifest))
        }
        args::Command::RestoreDesktopItems { manifest } => {
            finish_desktop_items(desktop_items::run_restore_file(&manifest))
        }
        args::Command::ServeSession { pipe, client_pid, client_created } => {
            // The session server runs until the launching app exits (then it force-exits itself).
            // Reaching here with an Ok is the app-died path; an Err is a fatal setup failure.
            match session::run_serve_session(&pipe, client_pid, client_created) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("session server failed: {e}");
                    ExitCode::from(3)
                }
            }
        }
        args::Command::Unknown(verb) => {
            eprintln!("Unsupported helper operation: {verb}");
            ExitCode::from(2)
        }
    }
}

fn finish(result: Result<(), String>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Helper operation failed: {e}");
            ExitCode::from(3)
        }
    }
}

/// Like [`finish`] but maps a desktop-items batch failure to a CLASSIFIED exit code the unelevated
/// launcher turns back into a human reason — the only failure channel across `runas` that is not a
/// caller-controlled elevated write (codex 2026-07-17 P1). The reason is still printed for an
/// interactive/log run; only the CODE crosses the process boundary.
fn finish_desktop_items(result: Result<(), String>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Helper desktop-items operation failed: {e}");
            ExitCode::from(desktop_items::classify_failure(&e))
        }
    }
}
