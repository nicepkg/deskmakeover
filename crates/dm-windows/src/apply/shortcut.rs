//! `.lnk` apply + restore. Ported from `ShellLinkShortcutIconWriter` and the preflight in
//! `ShortcutIconApplyOperation` (`DesktopIconApplyOperations.cs`). The apply runs on the STA
//! thread (COM); restore is a plain byte replay.

use std::path::Path;

use dm_domain::{PortError, PortResult};

use crate::shell::{attrs, shell_link};

/// Points the shortcut at the generated `.ico`. Preflights that both the generated asset and the
/// shortcut still exist (oracle `PreflightTarget`); the durable CAS check lives in the driver.
///
/// [WINDOWS-VERIFY] runtime.
pub fn apply(shortcut_path: &str, icon_path: &str) -> PortResult<()> {
    // try_exists() distinguishes genuine absence from an access failure (audit F3): an unreadable but
    // PRESENT asset/shortcut must surface its real IO error, not masquerade as missing.
    match Path::new(icon_path).try_exists() {
        Ok(true) => {}
        Ok(false) => return Err(PortError::AssetMissing(icon_path.to_string())),
        Err(e) => return Err(PortError::Io(format!("cannot access {icon_path}: {e}"))),
    }
    match Path::new(shortcut_path).try_exists() {
        Ok(true) => {}
        Ok(false) => return Err(PortError::NotFound(shortcut_path.to_string())),
        Err(e) => return Err(PortError::Io(format!("cannot access {shortcut_path}: {e}"))),
    }
    // A .lnk swapped for a symlink since scan would have IPersistFile::Save follow it elsewhere. (APPLY-2)
    if attrs::is_reparse_point(shortcut_path)? {
        return Err(PortError::Io(format!(
            "{shortcut_path} became a reparse point after scan; refusing to write through it"
        )));
    }
    shell_link::set_icon_location(shortcut_path, icon_path, 0)
}

/// Restores the shortcut by writing its captured original bytes back verbatim
/// (oracle `RestoreOriginalContent`). Durable + atomic so a crash mid-restore can't tear the
/// `.lnk` (P1-9). [WINDOWS-VERIFY] runtime.
pub fn restore_bytes(path: &str, bytes: &[u8]) -> PortResult<()> {
    crate::durable::write_atomic(path, bytes)
}

/// The Store/MSIX wrapper apply (Task 11, ADR-0024): an EXISTING wrapper is the ordinary
/// IconLocation rewrite (`apply`, AUMID-preserving); an ABSENT one — the virtual AppsFolder entry
/// at its prospective path — is CREATED via the wrapper recipe, the AUMID resolved from the
/// path's file stem (the item's display name) against the live AppsFolder. A stem no AppsFolder
/// entry answers to is an honest NotFound (the app was uninstalled between scan and apply), never
/// a fabricated wrapper. [WINDOWS-VERIFY] runtime.
pub fn apply_appx(shortcut_path: &str, icon_path: &str) -> PortResult<()> {
    match Path::new(shortcut_path).try_exists() {
        Ok(true) => return apply(shortcut_path, icon_path),
        Ok(false) => {}
        Err(e) => return Err(PortError::Io(format!("cannot access {shortcut_path}: {e}"))),
    }
    // try_exists() distinguishes genuine absence from an access failure (audit F3): an unreadable
    // PRESENT asset must surface its real IO error, not masquerade as missing.
    match Path::new(icon_path).try_exists() {
        Ok(true) => {}
        Ok(false) => return Err(PortError::AssetMissing(icon_path.to_string())),
        Err(e) => return Err(PortError::Io(format!("cannot access {icon_path}: {e}"))),
    }
    let stem = Path::new(shortcut_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| PortError::Io(format!("no usable file stem on {shortcut_path}")))?;
    let aumid = crate::shell::appsfolder::resolve_aumid(stem)?.ok_or_else(|| {
        PortError::NotFound(format!("no AppsFolder entry named {stem:?} for {shortcut_path}"))
    })?;
    shell_link::create_appx_shortcut(shortcut_path, &aumid, icon_path)
}

/// Removes a wrapper `.lnk` whose original state was ABSENT (Task 11,
/// [`dm_domain::RestoreAnchor::FileAbsent`]): the restore of a virtual AppsFolder entry is "the
/// wrapper we created stops existing". The driver's CAS (live == last_applied) gates this call, so
/// the file being removed is the styled wrapper it verified. `try_exists` distinguishes absence
/// (already restored — nothing to do) from an access fault (propagated, P2-#3 posture).
/// [WINDOWS-VERIFY] runtime.
pub fn remove(path: &str) -> PortResult<()> {
    match Path::new(path).try_exists() {
        Ok(true) => {}
        Ok(false) => return Ok(()),
        Err(e) => return Err(PortError::Io(format!("cannot access {path}: {e}"))),
    }
    std::fs::remove_file(path).map_err(|e| PortError::Io(format!("cannot remove {path}: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::com::StaExecutor;

    #[test]
    fn remove_is_idempotent_on_absence_and_honest_about_dirs() {
        let dir = tempfile::tempdir().unwrap();
        // Absent → already restored, not an error (the CAS-gated restore may race a prior reset).
        let missing = dir.path().join("never.lnk");
        remove(&missing.to_string_lossy()).unwrap();

        // Present → removed.
        let present = dir.path().join("wrapper.lnk");
        std::fs::write(&present, b"w").unwrap();
        remove(&present.to_string_lossy()).unwrap();
        assert!(!present.exists(), "the wrapper is gone");

        // A DIRECTORY at the path → honest IO error, never a silent "restored".
        let adir = dir.path().join("adir.lnk");
        std::fs::create_dir(&adir).unwrap();
        assert!(matches!(remove(&adir.to_string_lossy()), Err(PortError::Io(_))));
    }

    #[test]
    fn apply_appx_preflights_the_asset_before_the_appsfolder_lookup() {
        // No COM: a missing asset on an ABSENT target must be AssetMissing (audit F3 — absence
        // must not mask an asset fault), reported BEFORE any AppsFolder enumeration runs.
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("Ghost App.lnk");
        let asset = dir.path().join("missing.ico");
        let err = apply_appx(&target.to_string_lossy(), &asset.to_string_lossy()).unwrap_err();
        assert!(matches!(err, PortError::AssetMissing(_)), "got {err:?}");
    }

    /// On-box (COM): the present branch is the ordinary AUMID-preserving rewrite; the absent
    /// branch with a stem NO AppsFolder entry answers to is an honest NotFound — never a
    /// fabricated wrapper (the app was uninstalled between scan and apply).
    #[test]
    fn apply_appx_rewrites_a_present_wrapper_and_degrades_honestly_for_a_ghost_stem() {
        let assets = tempfile::tempdir().unwrap();
        let icon_a = assets.path().join("a.ico");
        std::fs::write(&icon_a, b"a").unwrap();
        let icon_b = assets.path().join("b.ico");
        std::fs::write(&icon_b, b"b").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let lnk = dir.path().join("WhatsApp.lnk").to_string_lossy().into_owned();
        let icon_a = icon_a.to_string_lossy().into_owned();
        let icon_b = icon_b.to_string_lossy().into_owned();

        let exec = StaExecutor::spawn().unwrap();
        let (l, i) = (lnk.clone(), icon_a);
        exec.run(move || shell_link::create_appx_shortcut(&l, "Fake.App_abc!App", &i))
            .unwrap()
            .unwrap();

        // Present → the ordinary rewrite, onto icon_b.
        let (l, i) = (lnk.clone(), icon_b.clone());
        exec.run(move || apply_appx(&l, &i)).unwrap().unwrap();
        let l2 = lnk.clone();
        let loc = exec.run(move || shell_link::read_icon_location(&l2)).unwrap().unwrap();
        assert_eq!(loc, Some((icon_b.clone(), 0)), "the present branch rewrote the icon");

        // Absent + unresolvable stem → honest NotFound (live AppsFolder enumeration, no match).
        let ghost =
            dir.path().join("Definitely No Such App 9f31.lnk").to_string_lossy().into_owned();
        let err = exec.run(move || apply_appx(&ghost, &icon_b)).unwrap().unwrap_err();
        assert!(matches!(err, PortError::NotFound(_)), "got {err:?}");
    }
}
