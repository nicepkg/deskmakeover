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
    let tmp = extended_length_path(&crate::durable::temp_path_for(shortcut_path));
    // SAFETY: COM object confined to this STA thread; dropped before we publish, so the target is
    // no longer held open when ReplaceFileW runs.
    unsafe {
        let link: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(com)?;
        let file: IPersistFile = link.cast().map_err(com)?;
        file.Load(&HSTRING::from(target.as_str()), STGM_READWRITE).map_err(com)?;
        link.SetIconLocation(&HSTRING::from(icon_path), index).map_err(com)?;
        // Save to the temp path (fRemember = false); durable::finalize_saved publishes it.
        file.Save(&HSTRING::from(tmp.as_str()), false).map_err(com)?;
    }
    crate::durable::finalize_saved(&tmp, &target)
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
    let tmp = extended_length_path(&crate::durable::temp_path_for(out_lnk));
    // SAFETY: COM object created, used, and dropped on this STA thread before we publish.
    unsafe {
        let link: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(com)?;
        link.SetPath(&HSTRING::from(target)).map_err(com)?;
        link.SetWorkingDirectory(&HSTRING::from(working_dir)).map_err(com)?;
        link.SetIconLocation(&HSTRING::from(icon_path), 0).map_err(com)?;
        let file: IPersistFile = link.cast().map_err(com)?;
        // Save to a temp sibling, then flush + atomically publish over the target (P1-3).
        file.Save(&HSTRING::from(tmp.as_str()), false).map_err(com)?;
    }
    crate::durable::finalize_saved(&tmp, &out)
}

fn wide_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

fn com(e: windows::core::Error) -> PortError {
    PortError::Com(e.to_string())
}
