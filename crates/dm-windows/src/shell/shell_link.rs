//! `.lnk` reading and writing via `IShellLinkW` + `IPersistFile`. Ported from
//! `DeskMakeover.Shell/ShellLinkComInterop.cs`, `ShellLinkShortcutReader.cs`, and
//! `ShellLinkShortcutIconWriter.cs`. Must be called on the STA thread.

use dm_domain::{PortError, PortResult};
use windows::core::{Interface, HSTRING};
use windows::Win32::System::Com::{
    CoCreateInstance, IPersistFile, CLSCTX_INPROC_SERVER, STGM_READ, STGM_READWRITE,
};
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

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
        file.Load(&HSTRING::from(shortcut_path), STGM_READ).map_err(com)?;
        let mut buf = [0u16; ICON_BUF];
        let mut index = 0i32;
        link.GetIconLocation(&mut buf, &mut index).map_err(com)?;
        let location = wide_to_string(&buf);
        Ok(if location.is_empty() { None } else { Some((location, index)) })
    }
}

/// Points the `.lnk` at `icon_path` (with `index`) and persists it, remembering the change.
/// Mirrors `ShellLinkShortcutIconWriter.Apply`.
///
/// [WINDOWS-VERIFY] runtime.
pub fn set_icon_location(shortcut_path: &str, icon_path: &str, index: i32) -> PortResult<()> {
    // SAFETY: COM object confined to this STA thread.
    unsafe {
        let link: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(com)?;
        let file: IPersistFile = link.cast().map_err(com)?;
        file.Load(&HSTRING::from(shortcut_path), STGM_READWRITE).map_err(com)?;
        link.SetIconLocation(&HSTRING::from(icon_path), index).map_err(com)?;
        file.Save(&HSTRING::from(shortcut_path), true).map_err(com)?;
        Ok(())
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
        file.Load(&HSTRING::from(shortcut_path), STGM_READ).map_err(com)?;
        let mut buf = [0u16; ICON_BUF];
        link.GetPath(&mut buf, std::ptr::null_mut(), 0).map_err(com)?;
        let target = wide_to_string(&buf);
        Ok(if target.is_empty() { None } else { Some(target) })
    }
}

fn wide_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

fn com(e: windows::core::Error) -> PortError {
    PortError::Com(e.to_string())
}
