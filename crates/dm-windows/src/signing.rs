//! The A1/C3 signature gate: verify + PIN `dm-elevated.exe` before the `runas`.
//!
//! The M8 installer lands the app + its `dm-elevated` sidecar under a per-user, USER-WRITABLE dir
//! (`%LOCALAPPDATA%`). Because the app launches the helper ELEVATED (ShellExecuteEx `runas`), a
//! swapped helper would run as administrator — a user→admin local privilege escalation. Two things
//! close this:
//! * **verify** — the on-disk helper must carry a valid, trusted Authenticode signature
//!   (`WinVerifyTrust`, `WINTRUST_ACTION_GENERIC_VERIFY_V2`). A tampered/unsigned/revoked helper is
//!   REFUSED (fail-closed); the elevated verb never launches.
//! * **pin** — verification and the launch are separate operations against a user-writable path, so
//!   an attacker could swap the file BETWEEN the check and `ShellExecuteExW` (§P1-4 TOCTOU). The gate
//!   opens the helper with a **deny-write / deny-delete** share, verifies THAT open handle, and holds
//!   it across the launch — so the exact verified image cannot be replaced in the window.
//!
//! Residual (tracked follow-ups, [WINDOWS-VERIFY] on the signed box): signer-subject PINNING (verify
//! the signer is OUR OV certificate, not merely any trusted CA — needs the finalized cert subject,
//! STATE.md open question) and a per-machine (admin-protected) install location, which would remove
//! the user-writable-helper problem at its root.

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::path::Path;

    use dm_domain::{PortError, PortResult};
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, HANDLE, HWND, TRUST_E_NOSIGNATURE};
    use windows::Win32::Security::WinTrust::{
        WinVerifyTrust, WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_DATA_0,
        WINTRUST_FILE_INFO, WTD_CHOICE_FILE, WTD_REVOKE_NONE, WTD_STATEACTION_CLOSE,
        WTD_STATEACTION_VERIFY, WTD_UI_NONE,
    };
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, OPEN_EXISTING,
    };

    /// The dev-only escape hatch: allow launching an UNSIGNED helper (no signature at all) for local
    /// iteration on an unsigned `bun run tauri:dev` build. Compiled ONLY into debug builds
    /// (`cfg(debug_assertions)`) — a SHIPPED (release) build ignores it entirely and always rejects an
    /// unsigned helper (§P1-3). It never weakens the tamper check: a helper that IS signed but
    /// UNTRUSTED (revoked / broken chain / a swapped foreign signature) is ALWAYS refused.
    #[cfg(debug_assertions)]
    const DEV_BYPASS_ENV: &str = "DESKMAKEOVER_ALLOW_UNSIGNED_HELPER";

    /// The elevated helper OPENED with a deny-write / deny-delete share and Authenticode-verified.
    /// Holding this across `ShellExecuteExW` pins the exact verified image — no other process can
    /// overwrite, rename, or delete it in the verify→launch window (§P1-4). Drop closes the handle.
    pub struct PinnedHelper {
        handle: HANDLE,
    }

    impl PinnedHelper {
        /// Opens `helper` (deny write + delete), verifies THAT handle's signature, and returns the
        /// held handle on success. A tampered/untrusted signature is ALWAYS refused; a total absence
        /// of signature is refused UNLESS the dev-bypass env var is set on a debug build.
        pub fn open_verified(helper: &Path) -> PortResult<Self> {
            let handle = open_denied_write(helper)?;
            let pin = PinnedHelper { handle }; // owns the handle now — Drop closes it on every path
            match verify_handle(handle, helper) {
                Verdict::Trusted => Ok(pin),
                Verdict::Unsigned => on_unsigned(helper, pin),
                Verdict::Untrusted(code) => Err(PortError::Com(format!(
                    "refusing to elevate helper {} — its signature is not trusted (WinVerifyTrust \
                     0x{code:08X}); a tampered or foreign-signed helper is never run elevated",
                    helper.display()
                ))),
            }
        }
    }

    impl Drop for PinnedHelper {
        fn drop(&mut self) {
            if !self.handle.is_invalid() {
                // SAFETY: `handle` came from CreateFileW and is closed exactly once (here).
                unsafe { let _ = CloseHandle(self.handle); }
            }
        }
    }

    #[cfg(debug_assertions)]
    fn on_unsigned(helper: &Path, pin: PinnedHelper) -> PortResult<PinnedHelper> {
        if std::env::var_os(DEV_BYPASS_ENV).is_some() {
            log::warn!(
                "elevated helper {} is UNSIGNED — allowed only because {DEV_BYPASS_ENV} is set on a \
                 DEBUG build (a shipped release build always rejects it)",
                helper.display()
            );
            Ok(pin)
        } else {
            Err(unsigned_error(helper))
        }
    }

    #[cfg(not(debug_assertions))]
    fn on_unsigned(helper: &Path, _pin: PinnedHelper) -> PortResult<PinnedHelper> {
        // Release builds ship a signed helper and NEVER honour a bypass (§P1-3).
        Err(unsigned_error(helper))
    }

    fn unsigned_error(helper: &Path) -> PortError {
        PortError::Com(format!(
            "refusing to elevate an UNSIGNED helper {} — the shipped helper is Authenticode-signed",
            helper.display()
        ))
    }

    /// Opens the helper for read with a deny-write / deny-delete share (`FILE_SHARE_READ` only), so
    /// no other process can replace it while the handle is held. Executing an image held this way is
    /// permitted by the loader (it maps the same read-shared file).
    fn open_denied_write(helper: &Path) -> PortResult<HANDLE> {
        let wide = encode_wide_nul(helper);
        // SAFETY: `wide` is a valid NUL-terminated buffer living across the call; no template handle.
        let handle = unsafe {
            CreateFileW(
                PCWSTR(wide.as_ptr()),
                GENERIC_READ.0,
                FILE_SHARE_READ, // deny FILE_SHARE_WRITE + FILE_SHARE_DELETE → cannot be swapped
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
        }
        .map_err(|e| PortError::Io(format!("cannot open the elevated helper to verify it: {e}")))?;
        Ok(handle)
    }

    enum Verdict {
        Trusted,
        Unsigned,
        Untrusted(u32),
    }

    /// Runs `WinVerifyTrust` against the ALREADY-OPEN handle (plus the path, per the API), so the
    /// bytes verified are the bytes pinned by `handle`.
    fn verify_handle(handle: HANDLE, helper: &Path) -> Verdict {
        let wide = encode_wide_nul(helper);
        let mut file_info = WINTRUST_FILE_INFO {
            cbStruct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
            pcwszFilePath: PCWSTR(wide.as_ptr()),
            hFile: handle,
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
        // suppressed (WTD_UI_NONE) so the null HWND is correct. The paired CLOSE call frees the state
        // WinVerifyTrust allocated regardless of the verify result.
        let status =
            unsafe { WinVerifyTrust(HWND::default(), &mut action, &mut data as *mut _ as *mut c_void) };
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

    fn encode_wide_nul(path: &Path) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        path.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
    }
}

#[cfg(windows)]
pub use imp::PinnedHelper;

/// Non-Windows stub so the crate links for the host cross-check; the gate only runs on Windows.
#[cfg(not(windows))]
pub struct PinnedHelper;

#[cfg(not(windows))]
impl PinnedHelper {
    pub fn open_verified(_helper: &std::path::Path) -> dm_domain::PortResult<Self> {
        Ok(PinnedHelper)
    }
}
