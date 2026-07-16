//! The A1/C3 signature gate: verify `dm-elevated.exe`'s Authenticode signature BEFORE the `runas`.
//!
//! The M8 installer lands the app + its `dm-elevated` sidecar under a per-user, USER-WRITABLE dir
//! (`%LOCALAPPDATA%`). Because the app launches the helper ELEVATED (ShellExecuteEx `runas`), a
//! swapped helper would run as administrator — a user→admin local privilege escalation. So every
//! `runas` first confirms the on-disk helper carries a valid, trusted Authenticode signature
//! (`WinVerifyTrust`, `WINTRUST_ACTION_GENERIC_VERIFY_V2`). A tampered/unsigned/revoked helper is
//! REFUSED (fail-closed) — the elevated verb never launches.
//!
//! Signer-subject PINNING (verify the signer is OUR certificate, not merely any trusted CA) is the
//! stronger guarantee and a tracked follow-up: it needs the finalized OV cert subject (STATE.md open
//! question). Until then this closes the unsigned/tampered/untrusted-CA class; a trusted-CA-signed
//! swap is the residual (documented) gap.
//!
//! [WINDOWS-VERIFY]: the live verdict is confirmed on the on-box SIGNED build (this is why the gate
//! honours `DESKMAKEOVER_ALLOW_UNSIGNED_HELPER=1` for LOCAL UNSIGNED dev iteration — see below).

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::path::Path;

    use dm_domain::{PortError, PortResult};
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{HWND, TRUST_E_NOSIGNATURE};
    use windows::Win32::Security::WinTrust::{
        WinVerifyTrust, WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_DATA_0,
        WINTRUST_FILE_INFO, WTD_CHOICE_FILE, WTD_REVOKE_NONE, WTD_STATEACTION_CLOSE,
        WTD_STATEACTION_VERIFY, WTD_UI_NONE,
    };

    /// The dev-only escape hatch: allow launching an UNSIGNED helper (no signature at all) for local
    /// iteration on an unsigned `bun run tauri:dev` build. It does NOT weaken the tamper check — a
    /// helper that IS signed but UNTRUSTED (revoked / broken chain / a swapped foreign signature) is
    /// ALWAYS refused, bypass or not. Production installers ship a signed helper, so this is never set.
    const DEV_BYPASS_ENV: &str = "DESKMAKEOVER_ALLOW_UNSIGNED_HELPER";

    /// Verifies `helper` carries a valid, trusted Authenticode signature. `Ok(())` ⇒ safe to `runas`.
    /// A tampered/untrusted/revoked signature is ALWAYS refused; a total absence of signature is
    /// refused UNLESS the dev-bypass env var is set (local unsigned builds only).
    pub fn verify_trusted_helper(helper: &Path) -> PortResult<()> {
        match verify_status(helper) {
            Verdict::Trusted => Ok(()),
            Verdict::Unsigned => {
                if std::env::var_os(DEV_BYPASS_ENV).is_some() {
                    log::warn!(
                        "elevated helper {} is UNSIGNED — allowed only because {DEV_BYPASS_ENV} is set \
                         (LOCAL DEV ONLY; a shipped build is always signed)",
                        helper.display()
                    );
                    Ok(())
                } else {
                    Err(PortError::Com(format!(
                        "refusing to elevate an UNSIGNED helper {} — the shipped helper is Authenticode-signed; \
                         set {DEV_BYPASS_ENV}=1 only for a local unsigned dev build",
                        helper.display()
                    )))
                }
            }
            Verdict::Untrusted(code) => Err(PortError::Com(format!(
                "refusing to elevate helper {} — its signature is not trusted (WinVerifyTrust 0x{code:08X}); \
                 a tampered or foreign-signed helper is never run elevated",
                helper.display()
            ))),
        }
    }

    enum Verdict {
        Trusted,
        Unsigned,
        Untrusted(u32),
    }

    fn verify_status(helper: &Path) -> Verdict {
        // A wide, NUL-terminated path buffer that outlives the two WinVerifyTrust calls.
        let wide: Vec<u16> = helper.as_os_str().encode_wide_nul();
        let mut file_info = WINTRUST_FILE_INFO {
            cbStruct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
            pcwszFilePath: PCWSTR(wide.as_ptr()),
            ..Default::default()
        };
        let mut data = WINTRUST_DATA {
            cbStruct: std::mem::size_of::<WINTRUST_DATA>() as u32,
            dwUIChoice: WTD_UI_NONE,
            fdwRevocationChecks: WTD_REVOKE_NONE,
            dwUnionChoice: WTD_CHOICE_FILE,
            dwStateAction: WTD_STATEACTION_VERIFY,
            Anonymous: WINTRUST_DATA_0 { pFile: &mut file_info },
            ..Default::default()
        };
        let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
        // SAFETY: `action` + `data` (and the buffers they point at) live across the call; UI is
        // suppressed (WTD_UI_NONE) so the null HWND is correct. The paired CLOSE call below frees the
        // state WinVerifyTrust allocated regardless of the verify result.
        let status = unsafe {
            WinVerifyTrust(
                HWND::default(),
                &mut action,
                &mut data as *mut _ as *mut c_void,
            )
        };
        data.dwStateAction = WTD_STATEACTION_CLOSE;
        // SAFETY: same struct, now asking WinVerifyTrust to release the state it allocated.
        unsafe {
            WinVerifyTrust(HWND::default(), &mut action, &mut data as *mut _ as *mut c_void);
        }
        if status == 0 {
            Verdict::Trusted
        } else if status as u32 == TRUST_E_NOSIGNATURE.0 as u32 {
            Verdict::Unsigned
        } else {
            Verdict::Untrusted(status as u32)
        }
    }

    /// `OsStr` → NUL-terminated UTF-16, the shape the Win32 wide-string APIs want.
    trait EncodeWideNul {
        fn encode_wide_nul(&self) -> Vec<u16>;
    }
    impl EncodeWideNul for std::ffi::OsStr {
        fn encode_wide_nul(&self) -> Vec<u16> {
            use std::os::windows::ffi::OsStrExt;
            self.encode_wide().chain(std::iter::once(0)).collect()
        }
    }
}

#[cfg(windows)]
pub use imp::verify_trusted_helper;

/// Non-Windows stub so the crate links for the host cross-check; the gate only runs on Windows.
#[cfg(not(windows))]
pub fn verify_trusted_helper(_helper: &std::path::Path) -> dm_domain::PortResult<()> {
    Ok(())
}
