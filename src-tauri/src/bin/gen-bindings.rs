//! Regenerate `src/bridge/generated.ts` from the Rust command surface.
//! Invoked by `bun run gen:bindings`. Writing lives in a binary (not a test) so a
//! plain `cargo test` never mutates the source tree.

fn main() {
    let path = deskmakeover_desktop_lib::bindings_path();
    deskmakeover_desktop_lib::export_bindings(&path).expect("failed to export TS bindings");
    println!("wrote {}", path.display());
}
