// Embed the requireAdministrator manifest into the helper binary (ADR-0019 Amendment 1)
// so a direct launch — and the app's ShellExecuteExW "runas" — always elevates via UAC.
//
// The MSVC linker auto-generates a UAC manifest fragment defaulting to level='asInvoker';
// merging our requireAdministrator manifest on top of that fails mt.exe with a conflicting
// `level` attribute (LNK1327). /MANIFESTUAC:NO suppresses the linker's own UAC fragment so
// only our manifest's requestedExecutionLevel survives. The link-args are emitted only for a
// Windows target — `cargo check --target x86_64-pc-windows-msvc` on a non-Windows host never
// links, so the host msvc cross-check stays clean (no C / no resource script runs).
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("dm-elevated.exe.manifest");
        println!("cargo:rerun-if-changed=dm-elevated.exe.manifest");
        println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg-bins=/MANIFESTUAC:NO");
        println!("cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}", manifest.display());
    }
}
