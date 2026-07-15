//! Known-folder resolution for the user + public desktops (oracle: `DesktopPaths.Current`).
//! Paths are resolved via `SHGetKnownFolderPath`, never hardcoded (spec 07 §3).

use std::path::PathBuf;

use dm_domain::{PortError, PortResult};
use windows::core::GUID;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{
    SHGetKnownFolderPath, FOLDERID_Desktop, FOLDERID_ProgramData, FOLDERID_PublicDesktop,
    KF_FLAG_DEFAULT,
};

/// The existing desktop roots (user first, then public). Missing roots are skipped.
///
/// [WINDOWS-VERIFY] runtime.
pub fn desktop_roots() -> PortResult<Vec<PathBuf>> {
    let mut roots = Vec::new();
    for id in [&FOLDERID_Desktop, &FOLDERID_PublicDesktop] {
        // One folder's lookup failing (e.g. FOLDERID_PublicDesktop unresolved on a
        // locked-down profile/edition) must NOT discard an already-resolved user Desktop
        // and fail the whole scan — the doc promise "missing roots are skipped" has to hold
        // for an API-level error, not only a physically-absent folder. Skip that root. (SHELL-1)
        match known_folder(id) {
            Ok(Some(path)) if path.is_dir() => roots.push(path),
            Ok(_) => {}
            Err(_) => continue,
        }
    }
    Ok(roots)
}

/// The §14 privileged-scope roots: `(Public Desktop, ProgramData)`, resolved via
/// `SHGetKnownFolderPath` (spec 07 §14, never hardcoded). Fail-closed contract: `None` when
/// EITHER folder does not resolve — the caller must then run with `ScopeRoots::Unresolved`
/// (defer everything) rather than a partial gate that silently fails open on the missing half.
///
/// [WINDOWS-VERIFY] runtime.
pub fn privileged_roots() -> Option<(PathBuf, PathBuf)> {
    let public = known_folder(&FOLDERID_PublicDesktop).ok().flatten()?;
    let programdata = known_folder(&FOLDERID_ProgramData).ok().flatten()?;
    Some((public, programdata))
}

fn known_folder(id: &GUID) -> PortResult<Option<PathBuf>> {
    // SAFETY: `SHGetKnownFolderPath` allocates a `PWSTR` that we own and free with
    // `CoTaskMemFree`; `to_string` reads the NUL-terminated buffer before it is freed.
    unsafe {
        let pwstr = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None)
            .map_err(|e| PortError::Com(e.to_string()))?;
        if pwstr.is_null() {
            return Ok(None);
        }
        // Free the buffer BEFORE propagating a decode error, or the `?` early-return leaks it.
        let text = pwstr.to_string();
        CoTaskMemFree(Some(pwstr.0 as *const core::ffi::c_void));
        Ok(Some(PathBuf::from(text.map_err(|e| PortError::Com(e.to_string()))?)))
    }
}
