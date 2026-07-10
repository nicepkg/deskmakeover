//! `dm-elevated` — the single privileged DeskMakeover binary (ADR-0019 Amendment 1, ADR-0021).
//!
//! It is a `requireAdministrator` helper whose ENTIRE job is the global shortcut-overlay verb
//! pair. There are no arbitrary commands and no scripting; the fixed verb set is parsed by
//! [`args`], the overlay ICO is validated by [`guards`], and the registry work lives in
//! [`overlay`]. Exit codes mirror the oracle `ElevatedHelper/Program.cs`: 0 success, 2 an
//! unknown/rejected verb, 3 an operation failure.
//!
//! The `requireAdministrator` elevation is applied at packaging (M8) via an external
//! `dm-elevated.exe.manifest`, so this crate has no resource-compiling build script and
//! cross-checks cleanly for `x86_64-pc-windows-msvc` on the host.

mod args;
mod guards;
mod overlay;
mod secure_dir;

use std::process::ExitCode;

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
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
