//! The client side of the elevated overlay verb pair: launch `dm-elevated` with the `runas`
//! verb (one UAC prompt), wait for it, and map its exit to an [`OverlayOutcome`]. Ported from the
//! `ElevatedOverlayBadgeService` invocation path; the privileged work itself is in the
//! `dm-elevated` crate (ADR-0021 §5: the overlay verb pair is the only v1 privileged surface).

use std::path::PathBuf;

use dm_domain::{OverlayControl, OverlayOutcome, OverlayStyle, PortError, PortResult};
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, ERROR_CANCELLED};
use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};

/// Invokes the elevated helper to apply/restore the global overlay.
pub struct WindowsOverlayControl {
    helper_path: PathBuf,
}

impl WindowsOverlayControl {
    pub fn new(helper_path: PathBuf) -> Self {
        Self { helper_path }
    }

    /// Resolves `dm-elevated.exe` beside the current executable (its per-machine install location).
    ///
    /// [WINDOWS-VERIFY] runtime.
    pub fn beside_current_exe() -> PortResult<Self> {
        let dir = std::env::current_exe()
            .map_err(|e| PortError::Io(e.to_string()))?
            .parent()
            .ok_or_else(|| PortError::Io("current exe has no parent directory".to_string()))?
            .to_path_buf();
        Ok(Self { helper_path: dir.join("dm-elevated.exe") })
    }

    fn run_helper(&self, params: &str) -> PortResult<OverlayOutcome> {
        let verb = HSTRING::from("runas");
        let file = HSTRING::from(self.helper_path.as_os_str());
        let parameters = HSTRING::from(params);

        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS,
            lpVerb: PCWSTR(verb.as_ptr()),
            lpFile: PCWSTR(file.as_ptr()),
            lpParameters: PCWSTR(parameters.as_ptr()),
            nShow: 0, // SW_HIDE — the console helper needs no window; UAC still prompts.
            ..Default::default()
        };

        // SAFETY: `info` is a fully-initialized SHELLEXECUTEINFOW living across the call; the
        // wide-string buffers it points at (`verb`/`file`/`parameters`) outlive the call.
        let launched = unsafe { ShellExecuteExW(&mut info) };
        if let Err(e) = launched {
            // A UAC cancellation surfaces as ERROR_CANCELLED → the user declined, not a failure.
            return if e.code() == windows::core::HRESULT::from_win32(ERROR_CANCELLED.0) {
                Ok(OverlayOutcome::Declined)
            } else {
                Err(PortError::Com(format!("ShellExecuteEx runas failed: {e}")))
            };
        }

        // SAFETY: `SEE_MASK_NOCLOSEPROCESS` populated `hProcess`; we wait on it, read its exit
        // code, then close the handle exactly once.
        let outcome = unsafe {
            WaitForSingleObject(info.hProcess, u32::MAX);
            let mut exit_code = 0u32;
            let read = GetExitCodeProcess(info.hProcess, &mut exit_code);
            let _ = CloseHandle(info.hProcess);
            match read {
                Ok(()) if exit_code == 0 => OverlayOutcome::Applied,
                Ok(()) => OverlayOutcome::Failed,
                Err(e) => return Err(PortError::Com(format!("GetExitCodeProcess failed: {e}"))),
            }
        };
        Ok(outcome)
    }
}

impl OverlayControl for WindowsOverlayControl {
    /// [WINDOWS-VERIFY] runtime (UAC + registry).
    fn apply(&self, style: OverlayStyle, ico_path: &str) -> PortResult<OverlayOutcome> {
        let style_arg = match style {
            OverlayStyle::Refined => "refined",
            OverlayStyle::Transparent => "transparent",
            OverlayStyle::Custom => "custom",
        };
        // The path is quoted in case it contains spaces.
        self.run_helper(&format!("apply-overlay --style {style_arg} --file \"{ico_path}\""))
    }

    /// [WINDOWS-VERIFY] runtime.
    fn restore(&self) -> PortResult<OverlayOutcome> {
        self.run_helper("restore-overlay")
    }
}
