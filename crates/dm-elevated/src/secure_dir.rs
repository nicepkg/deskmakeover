//! Hardened resolution of the elevated helper's `%ProgramData%` data directory (P1 LPE fix).
//!
//! ADR-0021 §4's LPE guard protected only the *value* the overlay registry key points at; it
//! trusted the `%ProgramData%\DeskMakeover` directory that value lives in. `C:\ProgramData` lets
//! a standard user create subdirectories (its ACL grants CREATOR OWNER on new children), so an
//! attacker could pre-create our subdir as a **junction** to, say, `System32` (junctions need no
//! privilege) and have the admin helper's `fs::write` land through the reparse point — an
//! arbitrary admin write — then pre-plant `overlay-state.txt` to defeat the one-time snapshot
//! guard and steer a later restore into `HKLM`. This module closes the hole:
//!
//! * the root comes from `SHGetKnownFolderPath(FOLDERID_ProgramData)`, never the forgeable
//!   `ProgramData` environment variable (folds in the P3 env-var finding);
//! * a **missing** subdir is created with a protected, restrictive DACL (SYSTEM + Administrators
//!   full, Users read/execute only, no inherited ACEs) via `CREATE` semantics that fail on a
//!   lost race — so a standard user can never end up able to write inside it;
//! * an **existing** subdir is trusted only when it is NOT a reparse point AND is owned by
//!   Administrators or SYSTEM (a standard user cannot set either owner without
//!   `SeRestorePrivilege`); anything else is refused rather than written through.
//!
//! The trust *policy* ([`dir_verdict`]) is pure and unit-tested on the host; the Windows glue that
//! gathers the facts and enforces the DACL ([`secure_data_dir`]) is [WINDOWS-VERIFY].

/// The SDDL for the data directory's DACL: `P`rotected (no ACEs inherited from
/// `C:\ProgramData`'s CREATOR OWNER grant), SYSTEM + Builtin Administrators full control, Builtin
/// Users read + execute only (`0x1200A9` = `FILE_GENERIC_READ | FILE_GENERIC_EXECUTE`, so the
/// shell can read the overlay ICOs to render them, but no non-admin can write).
pub const DATA_DIR_SDDL: &str = "D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;0x1200a9;;;BU)";

/// Facts a platform adapter gathers about the existing directory before the helper trusts it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirFacts {
    pub exists: bool,
    pub is_reparse_point: bool,
    pub owner_is_admin_or_system: bool,
}

/// What to do with the data directory given [`DirFacts`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirVerdict {
    /// Missing — create it fresh with the restrictive DACL (we then own it and it is non-reparse).
    CreateFresh,
    /// Present and trustworthy — use as-is.
    Trust,
    /// Present but suspicious — refuse; the message names why.
    Refuse(&'static str),
}

/// The pure trust policy. A present directory is trusted ONLY when it is not a reparse point and
/// is owned by Administrators/SYSTEM, so the admin helper never writes through a standard-user
/// junction nor reads a planted state file. Reparse is checked before owner because a junction is
/// the more dangerous signal (it redirects our writes elsewhere entirely).
pub fn dir_verdict(facts: DirFacts) -> DirVerdict {
    if !facts.exists {
        return DirVerdict::CreateFresh;
    }
    if facts.is_reparse_point {
        return DirVerdict::Refuse("data dir is a reparse point (possible junction/symlink attack)");
    }
    if !facts.owner_is_admin_or_system {
        return DirVerdict::Refuse("data dir owner is not Administrators or SYSTEM");
    }
    DirVerdict::Trust
}

#[cfg(windows)]
pub use windows_impl::secure_data_dir;

#[cfg(windows)]
mod windows_impl {
    use std::path::{Path, PathBuf};

    use windows::core::HSTRING;
    use windows::Win32::Foundation::{ERROR_SUCCESS, HLOCAL, LocalFree};
    use windows::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, GetNamedSecurityInfoW,
        SDDL_REVISION_1, SE_FILE_OBJECT,
    };
    use windows::Win32::Security::{
        IsWellKnownSid, WinBuiltinAdministratorsSid, WinLocalSystemSid, OWNER_SECURITY_INFORMATION,
        PSECURITY_DESCRIPTOR, PSID, SECURITY_ATTRIBUTES,
    };
    use windows::Win32::Storage::FileSystem::{
        CreateDirectoryW, GetFileAttributesW, FILE_ATTRIBUTE_REPARSE_POINT, INVALID_FILE_ATTRIBUTES,
    };
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{SHGetKnownFolderPath, FOLDERID_ProgramData, KF_FLAG_DEFAULT};

    use super::{dir_verdict, DirFacts, DirVerdict, DATA_DIR_SDDL};

    /// Resolves `%ProgramData%\DeskMakeover`, ensuring it exists and is trustworthy. Returns an
    /// error (fail closed) rather than proceeding through an attacker-controlled directory.
    pub fn secure_data_dir() -> Result<PathBuf, String> {
        let dir = program_data_root()?.join("DeskMakeover");
        match dir_verdict(gather_facts(&dir)) {
            DirVerdict::CreateFresh => create_locked_dir(&dir)?,
            DirVerdict::Trust => {}
            DirVerdict::Refuse(why) => return Err(format!("refusing to use data dir: {why}")),
        }
        Ok(dir)
    }

    fn program_data_root() -> Result<PathBuf, String> {
        // SAFETY: SHGetKnownFolderPath allocates a PWSTR we own; read it, then free with
        // CoTaskMemFree before returning.
        unsafe {
            let pwstr = SHGetKnownFolderPath(&FOLDERID_ProgramData, KF_FLAG_DEFAULT, None)
                .map_err(|e| e.to_string())?;
            if pwstr.is_null() {
                return Err("FOLDERID_ProgramData resolved to null".to_string());
            }
            let text = pwstr.to_string();
            CoTaskMemFree(Some(pwstr.0 as *const core::ffi::c_void));
            text.map(PathBuf::from).map_err(|e| e.to_string())
        }
    }

    fn gather_facts(dir: &Path) -> DirFacts {
        let h = HSTRING::from(dir);
        // SAFETY: GetFileAttributesW reads a path and returns INVALID_FILE_ATTRIBUTES when the
        // path cannot be queried (treated as "does not exist" → create fresh, which then fails
        // closed if the path actually exists but was unreadable).
        let attrs = unsafe { GetFileAttributesW(&h) };
        if attrs == INVALID_FILE_ATTRIBUTES {
            return DirFacts { exists: false, is_reparse_point: false, owner_is_admin_or_system: false };
        }
        DirFacts {
            exists: true,
            is_reparse_point: attrs & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0,
            owner_is_admin_or_system: owner_is_admin_or_system(&h),
        }
    }

    fn owner_is_admin_or_system(path: &HSTRING) -> bool {
        let mut owner = PSID(core::ptr::null_mut());
        let mut psd = PSECURITY_DESCRIPTOR(core::ptr::null_mut());
        // SAFETY: GetNamedSecurityInfoW allocates a security descriptor (freed below) and points
        // `owner` into it; we only read `owner` while the descriptor is alive.
        let rc = unsafe {
            GetNamedSecurityInfoW(
                path,
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION,
                Some(&mut owner),
                None,
                None,
                None,
                &mut psd,
            )
        };
        if rc != ERROR_SUCCESS || owner.is_invalid() {
            return false;
        }
        // SAFETY: `owner` is a valid SID pointer into the live descriptor; IsWellKnownSid reads it.
        let trusted = unsafe {
            IsWellKnownSid(owner, WinBuiltinAdministratorsSid).as_bool()
                || IsWellKnownSid(owner, WinLocalSystemSid).as_bool()
        };
        // SAFETY: the descriptor was allocated by GetNamedSecurityInfoW with LocalAlloc semantics.
        unsafe {
            let _ = LocalFree(Some(HLOCAL(psd.0)));
        }
        trusted
    }

    fn create_locked_dir(dir: &Path) -> Result<(), String> {
        let mut psd = PSECURITY_DESCRIPTOR(core::ptr::null_mut());
        let sddl = HSTRING::from(DATA_DIR_SDDL);
        // SAFETY: builds a self-relative descriptor from our constant SDDL into `psd` (freed below).
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(&sddl, SDDL_REVISION_1, &mut psd, None)
                .map_err(|e| e.to_string())?;
        }
        let sa = SECURITY_ATTRIBUTES {
            nLength: core::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: psd.0,
            ..Default::default()
        };
        let h = HSTRING::from(dir);
        // SAFETY: creates a single directory with our DACL. CreateDirectoryW uses CREATE semantics
        // — it errors (ERROR_ALREADY_EXISTS) if the path appeared since gather_facts, so a race to
        // pre-plant a junction cannot make us trust it.
        let result = unsafe { CreateDirectoryW(&h, Some(&sa)) };
        // SAFETY: free the descriptor ConvertStringSecurityDescriptor... allocated.
        unsafe {
            let _ = LocalFree(Some(HLOCAL(psd.0)));
        }
        result.map_err(|e| format!("failed to create locked data dir: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(exists: bool, is_reparse_point: bool, owner_is_admin_or_system: bool) -> DirFacts {
        DirFacts { exists, is_reparse_point, owner_is_admin_or_system }
    }

    #[test]
    fn a_missing_dir_is_created_fresh() {
        assert_eq!(dir_verdict(facts(false, false, false)), DirVerdict::CreateFresh);
        // "missing" wins even if stale reparse/owner facts were somehow set.
        assert_eq!(dir_verdict(facts(false, true, false)), DirVerdict::CreateFresh);
    }

    #[test]
    fn an_admin_owned_plain_dir_is_trusted() {
        assert_eq!(dir_verdict(facts(true, false, true)), DirVerdict::Trust);
    }

    #[test]
    fn a_reparse_point_is_always_refused() {
        assert!(matches!(dir_verdict(facts(true, true, true)), DirVerdict::Refuse(_)));
        // Reparse is refused even when the owner looks fine AND takes precedence over owner.
        match dir_verdict(facts(true, true, false)) {
            DirVerdict::Refuse(why) => assert!(why.contains("reparse")),
            other => panic!("expected reparse refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_foreign_owned_dir_is_refused() {
        match dir_verdict(facts(true, false, false)) {
            DirVerdict::Refuse(why) => assert!(why.contains("owner")),
            other => panic!("expected owner refusal, got {other:?}"),
        }
    }

    #[test]
    fn the_dacl_is_protected_and_denies_non_admin_writes() {
        // Protected DACL blocks C:\ProgramData's inheritable CREATOR OWNER grant.
        assert!(DATA_DIR_SDDL.starts_with("D:P"));
        // SYSTEM and Administrators get full control.
        assert!(DATA_DIR_SDDL.contains("(A;OICI;FA;;;SY)"));
        assert!(DATA_DIR_SDDL.contains("(A;OICI;FA;;;BA)"));
        // Users get read/execute ONLY — never full access (that would reopen the write vector).
        assert!(DATA_DIR_SDDL.contains("0x1200a9;;;BU"));
        assert!(!DATA_DIR_SDDL.contains("FA;;;BU"));
    }
}
