//! Regenerate `frontend/src/bridge/generated.ts` from the Rust command surface.
//! Invoked by `bun run gen:bindings`. Writing lives here (not in a test) so a
//! plain `cargo test` never mutates the source tree.
//!
//! Not under `src/bin/` on purpose: the repo `.gitignore` ignores every `bin/`
//! directory (a .NET rule), which would silently drop this file from git.

fn main() {
    let path = deskmakeover_desktop_lib::bindings_path();
    deskmakeover_desktop_lib::export_bindings(&path).expect("failed to export TS bindings");
    println!("wrote {}", path.display());
}
