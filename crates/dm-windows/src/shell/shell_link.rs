//! `.lnk` reading and writing via `IShellLinkW` + `IPersistFile`. Ported from
//! `DeskMakeover.Shell/ShellLinkComInterop.cs`, `ShellLinkShortcutReader.cs`, and
//! `ShellLinkShortcutIconWriter.cs`. Must be called on the STA thread.

use dm_domain::{PortError, PortResult};
use windows::core::{Interface, HSTRING};
use windows::Win32::System::Com::{
    CoCreateInstance, IPersistFile, CLSCTX_INPROC_SERVER, STGM_READ, STGM_READWRITE,
};
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

use crate::classify::extended_length_path;

const ICON_BUF: usize = 1024;

/// Reads the `.lnk`'s explicit icon location `(path, index)`, or `None` when it has none.
///
/// [WINDOWS-VERIFY] runtime.
pub fn read_icon_location(shortcut_path: &str) -> PortResult<Option<(String, i32)>> {
    // SAFETY: the shell-link COM object is created, used, and dropped on this (STA) thread.
    unsafe {
        let link: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(com)?;
        let file: IPersistFile = link.cast().map_err(com)?;
        let path = extended_length_path(shortcut_path);
        file.Load(&HSTRING::from(path.as_str()), STGM_READ).map_err(com)?;
        let mut buf = [0u16; ICON_BUF];
        let mut index = 0i32;
        link.GetIconLocation(&mut buf, &mut index).map_err(com)?;
        let location = wide_to_string(&buf);
        Ok(if location.is_empty() { None } else { Some((location, index)) })
    }
}

/// Points the `.lnk` at `icon_path` (with `index`) and persists it durably + atomically: the edit
/// is saved to a temp sibling, then flushed and published over the target, so a crash mid-write
/// cannot tear the live shortcut and a clean commit is durable (P1-3). Mirrors
/// `ShellLinkShortcutIconWriter.Apply`.
///
/// [WINDOWS-VERIFY] runtime.
pub fn set_icon_location(shortcut_path: &str, icon_path: &str, index: i32) -> PortResult<()> {
    let target = extended_length_path(shortcut_path);
    // Pre-claim the temp as a proven-regular O_EXCL file so Save can't be tricked into
    // creating/truncating through a pre-placed symlink (handoff §8a #1).
    let tmp = extended_length_path(&crate::durable::claim_temp_for(shortcut_path)?);
    // A failed Save can return before finalize_saved runs, leaving a partial temp sibling — never
    // strand it (new-P3). finalize_saved cleans up on ITS own failure.
    let saved = save_icon_to_temp(&target, &tmp, icon_path, index);
    if saved.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    saved?;
    crate::durable::finalize_saved(&tmp, &target)
}

/// Loads the `.lnk`, sets its icon location, and Saves it to `tmp` (fRemember = false). Split out so
/// the caller can clean up the temp on any failure. [WINDOWS-VERIFY] runtime.
fn save_icon_to_temp(target: &str, tmp: &str, icon_path: &str, index: i32) -> PortResult<()> {
    // SAFETY: COM object confined to this STA thread; dropped before we publish, so the target is
    // no longer held open when ReplaceFileW runs.
    unsafe {
        let link: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(com)?;
        let file: IPersistFile = link.cast().map_err(com)?;
        file.Load(&HSTRING::from(target), STGM_READWRITE).map_err(com)?;
        link.SetIconLocation(&HSTRING::from(icon_path), index).map_err(com)?;
        file.Save(&HSTRING::from(tmp), false).map_err(com)
    }
}

/// The identity fields a loose-file wrapper `.lnk` carries: its icon location, its resolved target,
/// and its working directory. Read together in one COM `Load` so the reader can fingerprint the
/// whole styleable surface a wrapper apply establishes (P1-#1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrapperIdentity {
    pub icon: Option<(String, i32)>,
    pub target: String,
    pub working_dir: String,
}

/// Reads a wrapper `.lnk`'s icon location + target + working directory in a single COM `Load`.
/// [WINDOWS-VERIFY] runtime.
pub fn read_wrapper_identity(shortcut_path: &str) -> PortResult<WrapperIdentity> {
    // SAFETY: the shell-link COM object is created, used, and dropped on this (STA) thread.
    unsafe {
        let link: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(com)?;
        let file: IPersistFile = link.cast().map_err(com)?;
        let path = extended_length_path(shortcut_path);
        file.Load(&HSTRING::from(path.as_str()), STGM_READ).map_err(com)?;

        let mut icon_buf = [0u16; ICON_BUF];
        let mut index = 0i32;
        link.GetIconLocation(&mut icon_buf, &mut index).map_err(com)?;
        let icon_path = wide_to_string(&icon_buf);
        let icon = if icon_path.is_empty() { None } else { Some((icon_path, index)) };

        let mut target_buf = [0u16; ICON_BUF];
        link.GetPath(&mut target_buf, std::ptr::null_mut(), 0).map_err(com)?;
        let target = wide_to_string(&target_buf);

        let mut dir_buf = [0u16; ICON_BUF];
        link.GetWorkingDirectory(&mut dir_buf).map_err(com)?;
        let working_dir = wide_to_string(&dir_buf);

        Ok(WrapperIdentity { icon, target, working_dir })
    }
}

/// Reads the `.lnk`'s resolved target path (used to extract a clean icon when the link has no
/// explicit icon location). Mirrors `ShellLinkComInterop.GetTargetPath`.
///
/// [WINDOWS-VERIFY] runtime.
pub fn read_target(shortcut_path: &str) -> PortResult<Option<String>> {
    // SAFETY: COM object confined to this STA thread; the find-data out-param is unused (null).
    unsafe {
        let link: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(com)?;
        let file: IPersistFile = link.cast().map_err(com)?;
        let path = extended_length_path(shortcut_path);
        file.Load(&HSTRING::from(path.as_str()), STGM_READ).map_err(com)?;
        let mut buf = [0u16; ICON_BUF];
        link.GetPath(&mut buf, std::ptr::null_mut(), 0).map_err(com)?;
        let target = wide_to_string(&buf);
        Ok(if target.is_empty() { None } else { Some(target) })
    }
}

/// Creates a brand-new `.lnk` at `out_lnk` pointing at `target`, with `working_dir` and
/// `icon_path`. Used to wrap a loose file as a styled shortcut (oracle
/// `RegularFileWrapperWriter.Wrap`). [WINDOWS-VERIFY] runtime.
pub fn create_shortcut(
    out_lnk: &str,
    target: &str,
    working_dir: &str,
    icon_path: &str,
) -> PortResult<()> {
    let out = extended_length_path(out_lnk);
    // Pre-claim the temp as a proven-regular O_EXCL file (handoff §8a #1).
    let tmp = extended_length_path(&crate::durable::claim_temp_for(out_lnk)?);
    // A failed Save can strand a partial temp sibling before finalize_saved runs — never leave it
    // (new-P3).
    let saved = save_new_shortcut_to_temp(&tmp, target, working_dir, icon_path);
    if saved.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    saved?;
    crate::durable::finalize_saved(&tmp, &out)
}

/// The durable ownership signature stamped into a wrapper `.lnk`'s Description (codex
/// icons2-🟠6): reunification checks THIS, not structural shape, so a user's own Hidden+System
/// file beside a same-named `.lnk` is never mistaken for our wrapper, and our own wrapper is
/// never missed on a case/Unicode path variant.
pub const WRAPPER_MARKER: &str = "DeskMakeover:file-wrapper:v1";

/// Reads a `.lnk`'s Description (the wrapper ownership marker lives here). `None` when unset.
pub fn read_description(shortcut_path: &str) -> PortResult<Option<String>> {
    // SAFETY: COM object confined to this STA thread.
    unsafe {
        let link: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(com)?;
        let file: IPersistFile = link.cast().map_err(com)?;
        let path = extended_length_path(shortcut_path);
        file.Load(&HSTRING::from(path.as_str()), STGM_READ).map_err(com)?;
        let mut buf = [0u16; ICON_BUF];
        // GetDescription fills the buffer; an empty description is a legal (unmarked) `.lnk`.
        if link.GetDescription(&mut buf).is_err() {
            return Ok(None);
        }
        let desc = wide_to_string(&buf);
        Ok(if desc.is_empty() { None } else { Some(desc) })
    }
}

/// Creates a fresh `.lnk` (target/working-dir/icon) and Saves it to `tmp`. Split out so the caller
/// can clean up the temp on any failure. [WINDOWS-VERIFY] runtime.
fn save_new_shortcut_to_temp(
    tmp: &str,
    target: &str,
    working_dir: &str,
    icon_path: &str,
) -> PortResult<()> {
    // SAFETY: COM object created, used, and dropped on this STA thread before we publish.
    unsafe {
        let link: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(com)?;
        link.SetPath(&HSTRING::from(target)).map_err(com)?;
        link.SetWorkingDirectory(&HSTRING::from(working_dir)).map_err(com)?;
        link.SetIconLocation(&HSTRING::from(icon_path), 0).map_err(com)?;
        // The durable ownership marker: our wrappers are self-identifying, so reunification never
        // guesses from structure (codex icons2-🟠6).
        link.SetDescription(&HSTRING::from(WRAPPER_MARKER)).map_err(com)?;
        let file: IPersistFile = link.cast().map_err(com)?;
        // Save to a temp sibling, then flush + atomically publish over the target (P1-3).
        file.Save(&HSTRING::from(tmp), false).map_err(com)
    }
}

fn wide_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

fn com(e: windows::core::Error) -> PortError {
    PortError::Com(e.to_string())
}
