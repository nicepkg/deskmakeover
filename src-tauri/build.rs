fn main() {
    // Integration-test binaries (`cargo test`) do not receive the application
    // manifest that `tauri_build::build()` embeds into the shipped app, so they
    // load System32's comctl32 v5.82 and crash at startup with
    // STATUS_ENTRYPOINT_NOT_FOUND against tao/wry's comctl32 v6-only imports.
    // Embed a Common-Controls v6 manifest into TEST targets only (the main bin
    // is already covered by tauri_build). See tests/common-controls-v6.manifest.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("common-controls-v6.manifest");
        println!("cargo:rerun-if-changed=tests/common-controls-v6.manifest");
        println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-tests=/MANIFESTINPUT:{}",
            manifest.display()
        );
    }

    tauri_build::build();
}
