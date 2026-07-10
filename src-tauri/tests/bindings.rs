//! CI drift guard: the committed `generated.ts` must equal a fresh export from
//! the current command surface. `bun run check:bindings` runs this; when it
//! fails, `bun run gen:bindings` regenerates and the diff gets committed.

use std::fs;

#[test]
fn bindings_are_up_to_date() {
    let committed_path = deskmakeover_desktop_lib::bindings_path();
    let committed = fs::read_to_string(&committed_path).unwrap_or_else(|_| {
        panic!(
            "{} is missing — run `bun run gen:bindings`",
            committed_path.display()
        )
    });

    let tmp = std::env::temp_dir().join(format!("dm-bindings-check-{}.ts", std::process::id()));
    deskmakeover_desktop_lib::export_bindings(&tmp).expect("failed to export TS bindings");
    let fresh = fs::read_to_string(&tmp).unwrap();
    fs::remove_file(&tmp).ok();

    assert_eq!(
        committed, fresh,
        "bridge bindings are stale — run `bun run gen:bindings` and commit generated.ts"
    );
}
