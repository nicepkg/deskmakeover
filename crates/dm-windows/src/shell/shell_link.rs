//! `.lnk` reading and writing via `IShellLinkW` + `IPersistFile`. Ported from
//! `DeskMakeover.Shell/ShellLinkComInterop.cs`, `ShellLinkShortcutReader.cs`, and
//! `ShellLinkShortcutIconWriter.cs`. Must be called on the STA thread.

use dm_domain::{PortError, PortResult};
use windows::core::{Interface, BSTR, HSTRING, PWSTR};
use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::System::Com::StructuredStorage::{
    PROPVARIANT, PROPVARIANT_0, PROPVARIANT_0_0, PROPVARIANT_0_0_0,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoTaskMemAlloc, IPersistFile, CLSCTX_INPROC_SERVER, STGM_READ, STGM_READWRITE,
};
use windows::Win32::System::Variant::VT_LPWSTR;
use windows::Win32::UI::Shell::PropertiesSystem::{
    IPropertyStore, SHGetPropertyStoreFromParsingName, GPS_READWRITE,
};
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

use crate::classify::extended_length_path;

const ICON_BUF: usize = 1024;

/// `PKEY_AppUserModel_ID` ({9f4c2855-9f79-4b39-a8d0-e1d42de1d5f3}, pid 5) — the property the shell's
/// Start/merge logic reads to associate a shortcut with a package (Task 11). Defined locally with
/// the exact key windows-rs happens to expose under `Win32_Storage_EnhancedStorage`, so the wrapper
/// recipe needs no extra crate feature.
const PKEY_APPUSERMODEL_ID: PROPERTYKEY = PROPERTYKEY {
    fmtid: windows::core::GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3),
    pid: 5,
};

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

/// Reads the `.lnk`'s argument string, or `None` when it has none. The AppsFolder classification
/// (Task 11) reads `shell:AppsFolder\<AUMID>` out of this. [WINDOWS-VERIFY] runtime.
pub fn read_arguments(shortcut_path: &str) -> PortResult<Option<String>> {
    // SAFETY: COM object confined to this STA thread.
    unsafe {
        let link: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(com)?;
        let file: IPersistFile = link.cast().map_err(com)?;
        let path = extended_length_path(shortcut_path);
        file.Load(&HSTRING::from(path.as_str()), STGM_READ).map_err(com)?;
        let mut buf = [0u16; ICON_BUF];
        link.GetArguments(&mut buf).map_err(com)?;
        let args = wide_to_string(&buf);
        Ok((!args.is_empty()).then_some(args))
    }
}

/// Points the `.lnk` at `icon_path` (with `index`) and persists it durably + atomically: the edit
/// is saved to a temp sibling, then flushed and published over the target, so a crash mid-write
/// cannot tear the live shortcut and a clean commit is durable (P1-3). Mirrors
/// `ShellLinkShortcutIconWriter.Apply`.
///
/// [WINDOWS-VERIFY] runtime.
pub fn set_icon_location(shortcut_path: &str, icon_path: &str, index: i32) -> PortResult<()> {
    // Preserve the AUMID across the rewrite (Task 11): the property store is NOT guaranteed to
    // survive an `IPersistFile::Load`/`Save` cycle, and a dropped AUMID would silently UN-MERGE a
    // styled Store wrapper back into a duplicate Start menu entry. Read it up front (plain path,
    // best-effort: an ordinary shortcut simply has none); `save_icon_to_temp` re-attaches it to the
    // loaded link OBJECT so the single Save publishes link + AUMID together.
    let aumid = read_appusermodel_id(shortcut_path)?;
    let target = extended_length_path(shortcut_path);
    // Pre-claim the temp as a proven-regular O_EXCL file so Save can't be tricked into
    // creating/truncating through a pre-placed symlink (handoff §8a #1).
    let tmp = extended_length_path(&crate::durable::claim_temp_for(shortcut_path)?);
    // A failed Save can return before finalize_saved runs, leaving a partial temp sibling — never
    // strand it (new-P3). finalize_saved cleans up on ITS own failure.
    let saved = save_icon_to_temp(&target, &tmp, icon_path, index, aumid.as_deref());
    if saved.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    saved?;
    crate::durable::finalize_saved(&tmp, &target)
}

/// Loads the `.lnk`, sets its icon location — and, when `aumid` is `Some`, re-attaches the AUMID
/// to the loaded ShellLink OBJECT before the save — and Saves it to `tmp` (fRemember = false).
/// Split out so the caller can clean up the temp on any failure. [WINDOWS-VERIFY] runtime.
fn save_icon_to_temp(
    target: &str,
    tmp: &str,
    icon_path: &str,
    index: i32,
    aumid: Option<&str>,
) -> PortResult<()> {
    // SAFETY: COM object confined to this STA thread; dropped before we publish, so the target is
    // no longer held open when ReplaceFileW runs.
    unsafe {
        let link: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(com)?;
        let file: IPersistFile = link.cast().map_err(com)?;
        file.Load(&HSTRING::from(target), STGM_READWRITE).map_err(com)?;
        // Preserve the AUMID atomically with the icon rewrite: set it on the OBJECT, so the one
        // Save below serialises link + property block together. A by-path property write on the
        // `.tmp`-named temp is impossible (no shell property handler for `.tmp`) — and this way a
        // crash can never strand a saved-but-un-merged wrapper.
        if let Some(aumid) = aumid {
            set_appusermodel_id(&link, aumid)?;
        }
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

/// The durable ownership signature stamped into a Store/MSIX wrapper `.lnk`'s Description
/// (Task 11, ADR-0024). Deliberately distinct from [`WRAPPER_MARKER`]: reunification keys on
/// `WRAPPER_MARKER` and must never claim an appx wrapper as a file wrapper — the guarantee is the
/// marker, not a structural guess.
pub const APPX_WRAPPER_MARKER: &str = "DeskMakeover:appx-wrapper:v1";

/// Creates the Store/MSIX wrapper `.lnk` at `out_lnk` (Task 11, ADR-0024): target = the HOST
/// binary (resolved from `current_exe`), arguments = `--activate-aumid <AUMID>`, description =
/// the appx marker, icon = the generated asset, AND `System.AppUserModel.ID` = the AUMID in the
/// property store — the field the shell's Start/merge logic actually reads, so the wrapper MERGES
/// with the app's real Start entry instead of duplicating it. The launch route is the box-verified
/// fallback: this box's `explorer.exe shell:AppsFolder\…` recipe fails to activate (see
/// [`save_new_appx_to_temp`]), while `IApplicationActivationManager` works. Durability:
/// claim-temp + finalize, like every other writer. [WINDOWS-VERIFY] runtime (incl. the
/// click-to-launch gate — PASSED on-box via the launcher route 2026-10-13).
pub fn create_appx_shortcut(out_lnk: &str, aumid: &str, icon_path: &str) -> PortResult<()> {
    let out = extended_length_path(out_lnk);
    // Pre-claim the temp as a proven-regular O_EXCL file (handoff §8a #1). The AUMID rides the
    // SAME save (set on the link object inside `save_new_appx_to_temp`), so there is no second,
    // by-path write that could fail independently of the Save.
    let tmp = extended_length_path(&crate::durable::claim_temp_for(out_lnk)?);
    // A failed Save can strand a partial temp sibling — never leave it (same posture as
    // `create_shortcut`).
    let saved = save_new_appx_to_temp(&tmp, aumid, icon_path);
    if saved.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    saved?;
    crate::durable::finalize_saved(&tmp, &out)
}

/// The launch + marker + icon + AUMID fields of an appx wrapper, saved to `tmp` in ONE
/// `IPersistFile::Save`. [WINDOWS-VERIFY] runtime.
fn save_new_appx_to_temp(tmp: &str, aumid: &str, icon_path: &str) -> PortResult<()> {
    // SAFETY: COM object created, used, and dropped on this STA thread before we publish.
    unsafe {
        let link: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(com)?;
        // The launch recipe — the box-verified FALLBACK (Task 11 click gate, 2026-10-13): the
        // `explorer.exe shell:AppsFolder\<AUMID>` route (Windows' own recipe, still the documented
        // primary in ADR-0024) does NOT activate apps on this box — three CLI attempts and a real
        // wrapper click all failed to start the target while `IApplicationActivationManager`
        // returned S_OK and the app stayed up (pre-existing COM registration damage, same family
        // as the wallpaper `0x80040154`). So the wrapper targets the HOST binary as a pure
        // launcher: `--activate-aumid <AUMID>` activates and exits without booting the GUI (the
        // CLI fast-path in `src/main.rs`). The AUMID property below keeps the Start-menu merge
        // INDEPENDENT of the launch mechanism. Creates run in-host only, so `current_exe` is the
        // desktop host — the elevated helper only ever rewrites PRESENT wrappers.
        let launcher = std::env::current_exe()
            .map_err(|e| dm_domain::PortError::Io(format!("current_exe: {e}")))?;
        link.SetPath(&HSTRING::from(launcher.to_string_lossy().as_ref())).map_err(com)?;
        link.SetArguments(&HSTRING::from(format!("--activate-aumid {aumid}"))).map_err(com)?;
        link.SetIconLocation(&HSTRING::from(icon_path), 0).map_err(com)?;
        link.SetDescription(&HSTRING::from(APPX_WRAPPER_MARKER)).map_err(com)?;
        // The AUMID (`System.AppUserModel.ID`) — the field the shell's Start/merge logic actually
        // reads — set on the OBJECT so this single Save serialises link + property block together
        // (a by-path property write on the `.tmp`-named temp is impossible: `.tmp` has no shell
        // property handler, and the final publish happens only after this Save lands).
        set_appusermodel_id(&link, aumid)?;
        let file: IPersistFile = link.cast().map_err(com)?;
        file.Save(&HSTRING::from(tmp), false).map_err(com)
    }
}

/// Reads the `System.AppUserModel.ID` property (the AUMID) from a `.lnk`'s property store, or
/// `None` when it has none — an ordinary shortcut, the common case. Best-effort by contract: a
/// failure to OPEN or read the store reads as "no AUMID", so a preserve-attempt can never break an
/// ordinary apply; only a KNOWN AUMID failing to re-attach ([`set_appusermodel_id`], fail closed)
/// is an error.
///
/// Takes the PLAIN path: `SHGetPropertyStoreFromParsingName` does not address `\\?\` paths, and
/// only reads a real `.lnk` — a `.tmp`-named temp has no shell property handler (which is why the
/// WRITE side is object-attached, not by-path). [WINDOWS-VERIFY] runtime.
pub fn read_appusermodel_id(shortcut_path: &str) -> PortResult<Option<String>> {
    // SAFETY: property-store COM confined to this STA thread; the returned PROPVARIANT's Drop
    // (PropVariantClear) owns and frees its value.
    unsafe {
        let store: IPropertyStore =
            match SHGetPropertyStoreFromParsingName(&HSTRING::from(shortcut_path), None, GPS_READWRITE)
            {
                Ok(s) => s,
                Err(_) => return Ok(None),
            };
        let pv = match store.GetValue(&PKEY_APPUSERMODEL_ID) {
            Ok(pv) => pv,
            Err(_) => return Ok(None), // property absent — ordinary shortcut
        };
        // `PropVariantToBSTR` coerces the canonical VT_LPWSTR (and a BSTR variant, if some other
        // writer stored one) without hand-poking union fields.
        let text = match BSTR::try_from(&pv) {
            Ok(b) => b.to_string(),
            Err(_) => return Ok(None),
        };
        Ok((!text.is_empty()).then_some(text))
    }
}

/// Sets `System.AppUserModel.ID` = `aumid` on a LIVE ShellLink OBJECT (`IPropertyStore` QI →
/// SetValue → Commit), so the following `IPersistFile::Save` serialises link + property block
/// together — the canonical AUMID recipe, and the only one that works here: the save target is a
/// `.tmp`-named temp (no shell property handler, so a by-path `SHGetPropertyStoreFromParsingName`
/// cannot open it), and the atomicity means a crash can never strand a saved-but-un-merged
/// wrapper.
///
/// The PROPVARIANT is built as VT_LPWSTR — the property's canonical type
/// (`InitPropVariantFromString`'s shape) — over a CoTaskMem buffer, which is exactly what
/// `PropVariantClear` (the PROPVARIANT Drop) frees; `PROPVARIANT::from(&str)` would store VT_BSTR,
/// a type the shell's readers do not expect here. [WINDOWS-VERIFY] runtime.
fn set_appusermodel_id(link: &IShellLinkW, aumid: &str) -> PortResult<()> {
    // SAFETY: property-store QI confined to this STA thread; the PROPVARIANT owns its CoTaskMem
    // buffer (freed on Drop) and the store COPIES the value at SetValue, so the buffer only needs
    // to outlive the call.
    unsafe {
        let store: IPropertyStore = link.cast().map_err(com)?;
        let pv = lpwstr_propvariant(aumid)?;
        store.SetValue(&PKEY_APPUSERMODEL_ID, &pv).map_err(com)?;
        store.Commit().map_err(com)
    }
}

/// A VT_LPWSTR PROPVARIANT over a CoTaskMem copy of `wide` — the ownership pairing
/// `PropVariantClear` (the PROPVARIANT Drop) needs.
fn lpwstr_propvariant(wide: &str) -> PortResult<PROPVARIANT> {
    let mut buf: Vec<u16> = wide.encode_utf16().collect();
    buf.push(0);
    // SAFETY: the only fault path is allocation failure; the copy length is the Vec's own.
    unsafe {
        let mem = CoTaskMemAlloc(buf.len() * 2) as *mut u16;
        if mem.is_null() {
            return Err(PortError::Io(format!(
                "CoTaskMemAlloc failed for {} bytes",
                buf.len() * 2
            )));
        }
        std::ptr::copy_nonoverlapping(buf.as_ptr(), mem, buf.len());
        Ok(PROPVARIANT {
            Anonymous: PROPVARIANT_0 {
                Anonymous: core::mem::ManuallyDrop::new(PROPVARIANT_0_0 {
                    vt: VT_LPWSTR,
                    wReserved1: 0,
                    wReserved2: 0,
                    wReserved3: 0,
                    Anonymous: PROPVARIANT_0_0_0 { pwszVal: PWSTR(mem) },
                }),
            },
        })
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::com::StaExecutor;
    use std::sync::Arc;

    /// A throwaway STA per test — the writer/reader contract demands apartment thread.
    fn exec() -> Arc<StaExecutor> {
        Arc::new(StaExecutor::spawn().unwrap())
    }

    #[test]
    fn appx_wrapper_round_trips_args_marker_icon_and_aumid_and_survives_an_icon_rewrite() {
        let dir = tempfile::tempdir().unwrap();
        let lnk = dir.path().join("WhatsApp.lnk").to_string_lossy().into_owned();
        let icon_a = dir.path().join("a.ico");
        std::fs::write(&icon_a, b"icon-a").unwrap();
        let icon_a = icon_a.to_string_lossy().into_owned();
        let icon_b = dir.path().join("b.ico");
        std::fs::write(&icon_b, b"icon-b").unwrap();
        let icon_b = icon_b.to_string_lossy().into_owned();
        const AUMID: &str = "5319275A.WhatsAppDesktop_cv1g1gvanyjgm!App";

        let e = exec();
        let (l, i) = (lnk.clone(), icon_a.clone());
        e.run(move || create_appx_shortcut(&l, AUMID, &i)).unwrap().unwrap();

        let e = exec();
        let l = lnk.clone();
        let (target, args, desc, aumid, icon_loc) = e
            .run(move || {
                Ok::<_, PortError>((
                    read_target(&l)?,
                    read_arguments(&l)?,
                    read_description(&l)?,
                    read_appusermodel_id(&l)?,
                    read_icon_location(&l)?,
                ))
            })
            .unwrap()
            .unwrap();
        assert_eq!(
            args.as_deref(),
            Some("--activate-aumid 5319275A.WhatsAppDesktop_cv1g1gvanyjgm!App"),
            "the wrapper launches through the HOST's --activate-aumid fast-path"
        );
        let launcher = std::env::current_exe().unwrap().to_string_lossy().into_owned();
        assert_eq!(target.as_deref(), Some(launcher.as_str()), "target = the host binary");
        assert_eq!(desc.as_deref(), Some(APPX_WRAPPER_MARKER));
        assert_eq!(aumid.as_deref(), Some(AUMID), "the property store must carry the AUMID");
        assert_eq!(icon_loc, Some((icon_a.clone(), 0)));

        // THE Task 11 round-trip pin: a re-style (icon rewrite via Load→SetIconLocation→Save) must
        // NOT drop the AUMID, args, or marker — a dropped AUMID silently UN-MERGES the styled
        // wrapper back into a duplicate Start entry.
        let e = exec();
        let (l, i) = (lnk.clone(), icon_b.clone());
        e.run(move || set_icon_location(&l, &i, 0)).unwrap().unwrap();

        let e = exec();
        let l = lnk.clone();
        let (args, desc, aumid, icon_loc) = e
            .run(move || {
                Ok::<_, PortError>((
                    read_arguments(&l)?,
                    read_description(&l)?,
                    read_appusermodel_id(&l)?,
                    read_icon_location(&l)?,
                ))
            })
            .unwrap()
            .unwrap();
        assert_eq!(aumid.as_deref(), Some(AUMID), "the icon rewrite must preserve the AUMID");
        assert_eq!(
            args.as_deref(),
            Some("--activate-aumid 5319275A.WhatsAppDesktop_cv1g1gvanyjgm!App"),
            "the icon rewrite must preserve the launch recipe"
        );
        assert_eq!(desc.as_deref(), Some(APPX_WRAPPER_MARKER));
        assert_eq!(icon_loc, Some((icon_b, 0)), "the icon itself was rewritten");
    }

    #[test]
    fn an_ordinary_shortcut_has_no_aumid_and_the_two_markers_stay_distinct() {
        // Reunification keys on WRAPPER_MARKER: the appx marker must never equal it, or a Store
        // wrapper could be claimed as a file wrapper (and vice versa).
        assert_ne!(APPX_WRAPPER_MARKER, WRAPPER_MARKER);

        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("real.bin");
        std::fs::write(&target, b"x").unwrap();
        let target = target.to_string_lossy().into_owned();
        let lnk = dir.path().join("plain.lnk").to_string_lossy().into_owned();
        let icon = dir.path().join("i.ico");
        std::fs::write(&icon, b"icon").unwrap();
        let icon = icon.to_string_lossy().into_owned();

        let e = exec();
        let (l, t, i) = (lnk.clone(), target, icon);
        e.run(move || create_shortcut(&l, &t, "", &i)).unwrap().unwrap();

        let e = exec();
        let l = lnk.clone();
        let (aumid, args) = e
            .run(move || Ok::<_, PortError>((read_appusermodel_id(&l)?, read_arguments(&l)?)))
            .unwrap()
            .unwrap();
        assert_eq!(aumid, None, "an ordinary shortcut carries no AUMID");
        assert_eq!(args, None, "an ordinary shortcut carries no arguments");
    }
}
