//! The AppsFolder virtual folder (`shell:AppsFolder` — the Start menu's "All apps" namespace):
//! enumeration, display-name → AUMID resolution, and the `IShellItem` lookup behind the Store/MSIX
//! wrapper `.lnk` recipe (Task 11, ADR-0024). Must be called on the STA thread (every entry point
//! is COM).

use dm_domain::{PortError, PortResult};
use windows::core::{HSTRING, Interface, GUID};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, IBindCtx, CLSCTX_ALL,
    COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Shell::{
    BHID_EnumItems, IApplicationActivationManager, IEnumShellItems, IShellItem,
    IShellItemImageFactory, SHCreateItemFromParsingName, SHCreateItemFromRelativeName,
    SIGDN_DESKTOPABSOLUTEPARSING, SIGDN_NORMALDISPLAY, SIGDN_PARENTRELATIVEPARSING, SIGDN,
    ACTIVATEOPTIONS,
};

/// One AppsFolder entry: the display name the Start menu shows, and its activation id (the AUMID
/// for MSIX apps; a classic entry's generated AUMID or `{CLSID}\app.msc`-style parsing name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppsfolderEntry {
    pub display_name: String,
    pub aumid: String,
}

/// Enumerates every AppsFolder entry (display name + AUMID). Task 11's scan synthesis and the
/// display-name → AUMID resolution both read this list.
///
/// [WINDOWS-VERIFY] runtime.
pub fn enumerate() -> PortResult<Vec<AppsfolderEntry>> {
    let folder = folder_item()?;
    // `BHID_EnumItems` binds the folder's child enumerator (windows-rs exposes it as
    // `IEnumShellItems`).
    let enumerator: IEnumShellItems = unsafe {
        folder
            .BindToHandler(None::<&IBindCtx>, &BHID_EnumItems)
            .map_err(com)?
    };
    let mut out = Vec::new();
    loop {
        let mut batch: [Option<IShellItem>; 64] = std::array::from_fn(|_| None);
        let mut fetched = 0u32;
        // `Next` returns Ok on S_FALSE (end of enumeration) — the fetched count is the truth.
        unsafe { enumerator.Next(&mut batch, Some(&mut fetched)).map_err(com)? };
        if fetched == 0 {
            break;
        }
        for item in batch.iter().flatten() {
            // One unreadable entry is skipped, never a reason to fail the whole enumeration.
            if let Some(entry) = entry_of(item) {
                out.push(entry);
            }
        }
        if fetched < batch.len() as u32 {
            break;
        }
    }
    Ok(out)
}

/// The AUMID whose AppsFolder entry displays `name` (case-insensitive — the same folding the scan
/// dedupe uses; first match wins, which the scan's duplicate rule already made unique).
///
/// [WINDOWS-VERIFY] runtime.
pub fn resolve_aumid(name: &str) -> PortResult<Option<String>> {
    let wanted = name.to_lowercase();
    Ok(enumerate()?
        .into_iter()
        .find(|e| e.display_name.to_lowercase() == wanted)
        .map(|e| e.aumid))
}

/// The `IShellItem` for one AppsFolder entry, addressed by AUMID — what
/// `IShellItemImageFactory::GetImage` is then cast onto to read the package glyph. The relative
/// name form is tried first (the documented AppsFolder-child lookup); a full
/// `shell:AppsFolder\<AUMID>` parsing name is the fallback for ids the relative form rejects.
///
/// [WINDOWS-VERIFY] runtime.
pub fn shell_item(aumid: &str) -> PortResult<IShellItem> {
    let folder = folder_item()?;
    unsafe {
        match SHCreateItemFromRelativeName(&folder, &HSTRING::from(aumid), None::<&IBindCtx>) {
            Ok(item) => Ok(item),
            Err(relative_err) => {
                let full = format!("shell:AppsFolder\\{aumid}");
                SHCreateItemFromParsingName(&HSTRING::from(full), None).map_err(|e| {
                    PortError::Com(format!(
                        "AppsFolder item {aumid:?} not found (relative: {relative_err}; parsing: {e})"
                    ))
                })
            }
        }
    }
}

/// The package glyph for one AppsFolder entry: the EXACT image Explorer's Start menu renders for
/// the app, read from the shell item factory. This read is anchored in the PACKAGE, never in any
/// wrapper `.lnk` — so it cannot compound `Style(Style(orig))` (Task 11).
///
/// [WINDOWS-VERIFY] runtime.
pub fn package_glyph_image(aumid: &str) -> PortResult<Option<dm_domain::DecodedImage>> {
    let item = shell_item(aumid)?;
    let factory: IShellItemImageFactory = item.cast().map_err(com)?;
    crate::source::factory_icon_image(&factory)
}

/// Activates a Store/MSIX app by AUMID through `IApplicationActivationManager::ActivateApplication`
/// — the Task 11 wrapper's LAUNCH mechanism on boxes whose `explorer.exe shell:AppsFolder\…` route
/// is broken (verified on-box 2026-10-13: three CLI attempts AND a real wrapper click all failed
/// to start the app while this API returned S_OK and the app stayed up — pre-existing COM
/// registration damage on the owner's box, same family as the wallpaper `0x80040154`). Returns the
/// activated process id. Owns its COM init: an unattached thread gets an STA, an already
/// initialized apartment (any mode) simply proceeds.
///
/// The CLSID has no windows-rs constant (defined here); the IID rides the crate's interface type
/// and is the documented `2E941141-7F97-4756-BA1D-9DECDE894A3D` (the widely-circulated
/// `2E97EF3E-…` is wrong → E_NOINTERFACE). [WINDOWS-VERIFY] runtime.
pub fn activate_app(aumid: &str) -> PortResult<u32> {
    // CLSID_IApplicationActivationManager (shell32).
    const CLSID: GUID = GUID::from_u128(0x45BA127D_10A8_46EA_8AB7_56EA9078943C);
    // SAFETY: the manager is a trusted system COM object; the HSTRING arguments outlive the call
    // (the shell copies what it needs at the call boundary).
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let manager: IApplicationActivationManager =
            CoCreateInstance(&CLSID, None, CLSCTX_ALL).map_err(com)?;
        let id = HSTRING::from(aumid);
        // `ACTIVATEOPTIONS(0)` = AO_NONE (the crate carries no named constant for it).
        manager
            .ActivateApplication(&id, &HSTRING::new(), ACTIVATEOPTIONS(0))
            .map_err(com)
    }
}

/// The AppsFolder as a shell item (`shell:AppsFolder` is a parsing name the shell resolves).
fn folder_item() -> PortResult<IShellItem> {
    unsafe {
        SHCreateItemFromParsingName(&HSTRING::from("shell:AppsFolder"), None).map_err(com)
    }
}

fn entry_of(item: &IShellItem) -> Option<AppsfolderEntry> {
    let display_name = display_name_of(item, SIGDN_NORMALDISPLAY)?;
    // The AUMID rides the parsing name. Which SIGDN yields the bare AUMID (vs. a composed
    // `shell:AppsFolder\...` form) is shell-version behaviour, so both parsing forms are tried and
    // [`strip_appsfolder_prefix`] normalises the result — pinned on-box by the enumeration test.
    let raw = display_name_of(item, SIGDN_DESKTOPABSOLUTEPARSING)
        .or_else(|| display_name_of(item, SIGDN_PARENTRELATIVEPARSING))?;
    let aumid = strip_appsfolder_prefix(&raw).to_string();
    (!display_name.trim().is_empty() && !aumid.trim().is_empty())
        .then(|| AppsfolderEntry { display_name, aumid })
}

fn display_name_of(item: &IShellItem, sigdn: SIGDN) -> Option<String> {
    // SAFETY: COM on the STA thread; the returned PWSTR is CoTaskMem-owned and freed here.
    unsafe {
        let pw = item.GetDisplayName(sigdn).ok()?;
        let text = pw.to_string().ok();
        CoTaskMemFree(Some(pw.as_ptr() as *const _));
        text
    }
}

/// Strips a leading `shell:AppsFolder\` from a parsing name so the stored AUMID is bare in both
/// shapes. Pure — unit-tested on the Mac host.
fn strip_appsfolder_prefix(name: &str) -> &str {
    const PREFIX: &str = "shell:appsfolder\\";
    match name.get(..PREFIX.len()) {
        Some(p) if p.eq_ignore_ascii_case(PREFIX) => &name[PREFIX.len()..],
        _ => name,
    }
}

fn com(e: windows::core::Error) -> PortError {
    PortError::Com(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_the_appsfolder_prefix_case_insensitively_and_leaves_bare_aumids() {
        assert_eq!(
            strip_appsfolder_prefix(r"shell:AppsFolder\5319275A.WhatsAppDesktop_cv1g1gvanyjgm!App"),
            "5319275A.WhatsAppDesktop_cv1g1gvanyjgm!App",
        );
        assert_eq!(
            strip_appsfolder_prefix(r"SHELL:APPSFOLDER\App_abc!App"),
            "App_abc!App",
        );
        // A bare AUMID (or anything else) passes through untouched.
        assert_eq!(strip_appsfolder_prefix("Brave"), "Brave");
        assert_eq!(
            strip_appsfolder_prefix(r"{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\printmanagement.msc"),
            r"{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\printmanagement.msc",
        );
        // A multibyte name never panics (byte-prefix via `get`).
        assert_eq!(strip_appsfolder_prefix("日本語"), "日本語");
    }

    /// On-box pin (Task 11 ship gate): the enumeration resolves the known WhatsApp entry to the
    /// AUMID `Get-StartApps` reports, proving which SIGDN the AUMID rode and that the whole
    /// enumerate → resolve chain addresses the real AppsFolder.
    #[test]
    fn enumerates_the_real_appsfolder_and_resolves_whatsapps_aumid() {
        // Shell COM runs on the STA thread (crate discipline) — never on the raw test thread.
        let exec = crate::com::StaExecutor::spawn().unwrap();
        let entries = exec.run(super::enumerate).unwrap().expect("AppsFolder enumeration failed");
        assert!(entries.len() >= 20, "expected the AppsFolder to enumerate, got {}", entries.len());
        let whatsapp = entries.iter().find(|e| e.display_name == "WhatsApp");
        match whatsapp {
            Some(e) => assert_eq!(
                e.aumid, "5319275A.WhatsAppDesktop_cv1g1gvanyjgm!App",
                "WhatsApp's parsing name must be its canonical AUMID"
            ),
            None => eprintln!("WhatsApp not installed on this box; enumeration still returned {} entries", entries.len()),
        }
        // resolve_aumid is the same lookup the apply/extraction paths use.
        if let Some(e) = whatsapp {
            let name = e.display_name.clone();
            let resolved = exec.run(move || super::resolve_aumid(&name)).unwrap().unwrap();
            assert_eq!(resolved.as_deref(), Some(e.aumid.as_str()));
        }
        // Every entry carries both halves (a blank one would produce a dead wrapper).
        for e in &entries {
            assert!(!e.display_name.trim().is_empty() && !e.aumid.trim().is_empty(), "blank entry: {e:?}");
        }
    }
}
