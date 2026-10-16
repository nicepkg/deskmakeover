//! File-TYPE styling via the ProgId's per-user `DefaultIcon` (ADR-0004 D7 amendment 2026-10-16:
//! the association ban is lifted for every type EXCEPT images — the amendment answers the ban's
//! own rationale clause by clause). The single-value sibling of [`super::system`] /
//! [`super::drive`]: apply writes EXACTLY ONE value,
//! `HKCU\Software\Classes\<ProgId>\DefaultIcon` = `"<path>,0"`, and never a command verb
//! (`shell\open\command`), never `UserChoice`, never HKCR/HKLM — no launch path can change, no
//! elevation is needed (a per-user `Software\Classes` entry overrides the machine class entry in
//! the merged HKCR view the shell reads).
//!
//! The ProgId rides the item's `path`, so it is **validated as a single registry key component**
//! before any interpolation (the same defence-in-depth discipline as `system::parse_clsid`): a
//! `\`-bearing string could otherwise write an unrelated nested key, and a ledger replay must
//! never mutate a target it was not captured for. [WINDOWS-VERIFY] runtime — the exact per-user
//! `Software\Classes\<ProgId>\DefaultIcon` override of a machine `HKCR\<ProgId>\DefaultIcon`.

use dm_domain::{FileTypeAnchor, PortError, PortResult, RegistryValue};
use winreg::enums::{HKEY_CLASSES_ROOT, HKEY_CURRENT_USER};
use winreg::RegKey;

use crate::apply::reg_icon::{io, read_value, write_or_delete};

/// Validates the item path as the ProgId apply will write under `Software\Classes\`. STRICT by
/// design: a ProgId is ONE registry key component — no path separators (they would climb into an
/// arbitrary subkey), no control characters, no `.`/`..`, no surrounding whitespace, no empty
/// string, and a sane length ceiling. Anything else is `Unsupported`, never used to build a key.
pub fn prog_id_from_path(path: &str) -> PortResult<String> {
    if is_valid_prog_id(path) {
        Ok(path.to_string())
    } else {
        Err(PortError::Unsupported(format!("not a file-type ProgId path: {path:?}")))
    }
}

fn is_valid_prog_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 200
        && s == s.trim()
        && !s.contains('\\')
        && !s.contains('/')
        && s != "."
        && s != ".."
        && !s.chars().any(|c| c.is_control())
}

/// The per-user override key apply writes and restore tears down — the ONLY key this feature
/// ever writes. `HKCU` — current user only, so no UAC and no machine mutation.
fn user_key(prog_id: &str) -> String {
    format!(r"Software\Classes\{prog_id}\DefaultIcon")
}

/// The machine class `DefaultIcon` (merged HKCR view) — the effective icon when no per-user
/// override exists. Never written.
fn machine_key(prog_id: &str) -> String {
    format!(r"{prog_id}\DefaultIcon")
}

/// Reads the current effective `DefaultIcon` state for `prog_id` — both the restore anchor and the
/// source-extraction original. Prefers the per-user override; falls back to the HKCR merged class
/// default (so a type registered machine-wide still recovers its true original and restores only
/// the per-user value we created).
///
/// A missing key/value is a benign absence (`None` / `key_existed:false`); any OTHER error
/// PROPAGATES rather than collapsing to "absent" — a `read_current` that silently reported
/// absence for an unreadable key would let the restore anchor forget the user's real state
/// (the same P2-#3 discipline as the drives). [WINDOWS-VERIFY] runtime.
pub fn read_current(prog_id: &str) -> PortResult<FileTypeAnchor> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    match hkcu.open_subkey(&user_key(prog_id)) {
        Ok(key) => Ok(FileTypeAnchor {
            prog_id: prog_id.to_string(),
            key_existed: true,
            value: read_value(&key, "")?,
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(FileTypeAnchor {
            prog_id: prog_id.to_string(),
            key_existed: false,
            value: machine_value(prog_id)?,
        }),
        Err(e) => Err(PortError::Io(format!("open HKCU file-type DefaultIcon {prog_id}: {e}"))),
    }
}

/// The HKCR class-default `DefaultIcon` for `prog_id`, or `None` when absent. A non-NotFound
/// error propagates (P2-#3). `pub(crate)` so source extraction can recover the original for an
/// owned item whose per-user key is empty (our restore leaves it empty).
pub(crate) fn machine_value(prog_id: &str) -> PortResult<Option<RegistryValue>> {
    let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
    match hkcr.open_subkey(&machine_key(prog_id)) {
        Ok(key) => read_value(&key, ""),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(PortError::Io(format!("open machine file-type DefaultIcon {prog_id}: {e}"))),
    }
}

/// Points `prog_id`'s per-user `DefaultIcon` at the styled ICO (`"<path>,0"`). THE ONLY WRITE —
/// no verb, no `UserChoice`, no machine key (ADR-0004 D7 amendment 2026-10-16).
pub fn apply(prog_id: &str, icon: &str) -> PortResult<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(&user_key(prog_id)).map_err(io)?;
    key.set_value("", &format!("{icon},0")).map_err(io)?;
    Ok(())
}

/// Restores the captured per-user value (or removes the value we created when none existed),
/// preserving the value's original kind. Byte-identical posture of `system::restore`.
pub fn restore(anchor: &FileTypeAnchor) -> PortResult<()> {
    // Defence-in-depth against a corrupt/stale ledger: the anchor's ProgId is interpolated into
    // an HKCU key path, so it must be a valid single component (same gate apply used).
    prog_id_from_path(&anchor.prog_id)?;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if !anchor.key_existed {
        // We created this value; undo by removing ONLY the `(Default)` value we wrote — never
        // `delete_subkey_all`, which would destroy unrelated state another program added under
        // the same `Software\Classes\<ProgId>` key (the recycle-bin posture). A non-NotFound
        // failure PROPAGATES; a missing value is idempotent.
        match hkcu.open_subkey_with_flags(&user_key(&anchor.prog_id), winreg::enums::KEY_SET_VALUE) {
            Ok(key) => match key.delete_value("") {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(io(e)),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {} // key already gone
            Err(e) => return Err(io(e)),
        }
        return Ok(());
    }
    let (key, _) = hkcu.create_subkey(&user_key(&anchor.prog_id)).map_err(io)?;
    write_or_delete(&key, "", anchor.value.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prog_id_from_path_accepts_a_plain_progid() {
        assert_eq!(prog_id_from_path("txtfile").unwrap(), "txtfile");
        assert_eq!(prog_id_from_path("Word.Document.8").unwrap(), "Word.Document.8");
        assert_eq!(prog_id_from_path("WinRAR.ZIP").unwrap(), "WinRAR.ZIP");
    }

    #[test]
    fn prog_id_from_path_refuses_anything_that_could_climb_a_key_path() {
        // The ProgId is interpolated into `Software\Classes\<ProgId>\DefaultIcon` — a separator
        // here would write an arbitrary nested key (the codex System-review 🟠 posture).
        assert!(prog_id_from_path(r"exefile\shell\open\command").is_err(), "verb injection");
        assert!(prog_id_from_path("a/b").is_err(), "forward slash");
        assert!(prog_id_from_path("").is_err(), "empty");
        assert!(prog_id_from_path("  txtfile").is_err(), "leading space");
        assert!(prog_id_from_path("txtfile ").is_err(), "trailing space");
        assert!(prog_id_from_path(".").is_err(), "dot");
        assert!(prog_id_from_path("..").is_err(), "dot dot");
        assert!(prog_id_from_path("txt\u{0}file").is_err(), "control char");
        assert!(prog_id_from_path(&"x".repeat(401)).is_err(), "over-long");
    }

    #[test]
    fn key_paths_are_per_progid_and_per_user_only() {
        // The per-user `Software\Classes` root is what makes this in-host / no-UAC; the machine
        // key is HKCR's class root and is read ONLY.
        assert_eq!(user_key("txtfile"), r"Software\Classes\txtfile\DefaultIcon");
        assert_eq!(machine_key("txtfile"), r"txtfile\DefaultIcon");
        assert!(user_key("txtfile").starts_with(r"Software\Classes\"));
    }

    /// Removes the test-only ProgId subtree so the fixture starts (and ends) with NO per-user
    /// key — repeated runs and a crashed run behave identically.
    fn scrub_test_progid() {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let _ = hkcu.delete_subkey_all(r"Software\Classes\DeskMakeoverTest.FileType.9f3c");
    }

    #[test]
    fn apply_restore_round_trips_a_per_user_value_and_leaves_no_residue() {
        // Round-trip on the LIVE HKCU with a dedicated test ProgId (never a real association),
        // exactly the shape a real item takes: no per-user key before us → key_existed:false →
        // restore must REMOVE the value, leaving the machine class entry (here: none) untouched.
        const TEST_PROG: &str = "DeskMakeoverTest.FileType.9f3c";
        scrub_test_progid();

        let before = read_current(TEST_PROG).expect("readable");
        assert!(!before.key_existed, "fixture must start with no per-user key");
        assert!(before.value.is_none(), "fixture must start with no machine fallback");

        apply(TEST_PROG, r"C:\gen\styled-icon.ico").unwrap();
        let styled = read_current(TEST_PROG).unwrap();
        assert!(styled.key_existed, "our override now exists");
        assert_eq!(
            styled.value.as_ref().map(|v| v.raw.as_str()),
            Some(r"C:\gen\styled-icon.ico,0")
        );

        restore(&FileTypeAnchor {
            prog_id: TEST_PROG.into(),
            key_existed: false,
            value: None,
        })
        .unwrap();
        let after = read_current(TEST_PROG).unwrap();
        // Exactly `system::restore`'s posture: ONLY the value we wrote is removed; the now-empty
        // per-user key is left in place (another program may have added values to it), so the
        // post-state reads `key_existed:true, value:None` — inert, because HKCR merges HKCU's
        // per-VALUE presence only and the machine class entry still shows through.
        assert!(after.value.is_none(), "value removed — no residue left behind");
        // Idempotent: restoring again is not an error (a ledger replay after a crash).
        restore(&FileTypeAnchor {
            prog_id: TEST_PROG.into(),
            key_existed: false,
            value: None,
        })
        .unwrap();
        // A PRE-EXISTING per-user value is REWRITTEN verbatim, never deleted (the true-original
        // path: someone else's override is the user's real state).
        apply(TEST_PROG, r"C:\other\pre.ico").unwrap();
        let original = RegistryValue { raw: r"C:\other\pre.ico,0".into(), kind: 1 };
        restore(&FileTypeAnchor { prog_id: TEST_PROG.into(), key_existed: true, value: Some(original.clone()) })
            .unwrap();
        let restored = read_current(TEST_PROG).unwrap();
        assert_eq!(restored.value, Some(original), "byte-identical rewrite of the captured value");
        scrub_test_progid();
    }

    #[test]
    fn restore_refuses_a_poisoned_progid_before_touching_the_registry() {
        // The anchor's `prog_id` interpolates into an HKCU key path — a `\`-bearing value in a
        // corrupt ledger must be refused, never used to mutate an unrelated key.
        let poisoned = FileTypeAnchor {
            prog_id: r"exefile\shell\open\command".into(),
            key_existed: true,
            value: Some(RegistryValue { raw: "evil.exe,0".into(), kind: 1 }),
        };
        assert!(restore(&poisoned).is_err(), "poisoned ProgId must be refused");
    }
}
