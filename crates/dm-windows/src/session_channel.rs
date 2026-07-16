//! Client of the session-scoped elevated helper (owner 2026-07-17: "authorize once per app
//! launch"). On the first privileged operation this launches `dm-elevated serve-session` via one
//! `runas`/UAC and keeps it alive; every later operation is a framed round-trip over the named
//! pipe — no further prompts until the app exits (the helper watches this process and self-exits).
//!
//! Robustness: the channel is an OPTIMISATION over the per-operation `runas`. If the session cannot
//! be ESTABLISHED (launch error, the pipe never appears, a mid-session I/O fault), [`SessionElevated::send`]
//! returns `Err` and the caller falls back to its existing one-UAC-per-op path — the feature can
//! only ever reduce prompts, never break elevation. A user UAC-cancel at launch is reported as
//! `Declined` (a real decision), NOT an establish error, so it is not retried as a fallback.
//!
//! Wire framing mirrors [`dm_elevated`]'s server: request = `u32 argc` then `argc × (u32 len, bytes)`;
//! response = `u32 exit_code, u32 msg_len, msg bytes`. All little-endian, UTF-8.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use dm_domain::{PortError, PortResult};
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::{ERROR_CANCELLED, GENERIC_READ, GENERIC_WRITE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_MODE, OPEN_EXISTING,
};
use windows::Win32::System::Pipes::WaitNamedPipeW;
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};

use crate::signing::PinnedHelper;

/// The outcome of one session request.
pub enum SessionSend {
    /// The session executed the verb; `code` is the helper's classified exit (0 = ok).
    Ran { code: u8, message: String },
    /// The user cancelled the UAC prompt when the session was being launched — a real decision, not
    /// an error to fall back from.
    Declined,
}

/// A lazily-launched, session-lived elevated helper reached over a named pipe.
pub struct SessionElevated {
    helper_path: PathBuf,
    client_pid: u32,
    /// The live session's pipe name, or `None` before first launch / after the helper died.
    active: Mutex<Option<String>>,
}

impl SessionElevated {
    pub fn new(helper_path: PathBuf) -> Self {
        Self { helper_path, client_pid: unsafe { GetCurrentProcessId() }, active: Mutex::new(None) }
    }

    /// Send one framed argv to the elevated session, launching it (one UAC) if it is not up.
    /// `Ok(SessionSend)` = the request reached the helper (or the user declined the launch);
    /// `Err` = the session could NOT be established — the caller should fall back to per-op `runas`.
    pub fn send(&self, argv: &[&str]) -> PortResult<SessionSend> {
        // GATED OFF by default pending security hardening (codex 2026-07-17 REQUEST-CHANGES: the
        // PID-only identity binding is forgeable via PID reuse, the client does not authenticate the
        // pipe server, the DACL uses the elevated token's SID, and info.hProcess leaks). Until those
        // are fixed + re-reviewed, `send` reports "unavailable" so every caller uses the safe per-op
        // `runas`. Set DESKMAKEOVER_SESSION_ELEVATION=1 to exercise the WIP path in dev only.
        if std::env::var_os("DESKMAKEOVER_SESSION_ELEVATION").is_none() {
            return Err(PortError::Com("session elevation disabled (pending security hardening)".into()));
        }
        let mut active = self.active.lock().unwrap();
        for attempt in 0..2u8 {
            // Ensure a live session.
            if active.is_none() {
                match self.launch()? {
                    Launch::Started(name) => *active = Some(name),
                    Launch::Declined => return Ok(SessionSend::Declined),
                }
            }
            let name = active.as_ref().expect("just ensured").clone();
            match round_trip(&name, argv) {
                Ok((code, message)) => return Ok(SessionSend::Ran { code, message }),
                // The helper died / the pipe broke. Drop the dead session and, on the first attempt,
                // relaunch (a fresh UAC). A second failure is a real establish failure → fall back.
                Err(e) => {
                    *active = None;
                    if attempt == 1 {
                        return Err(PortError::Com(format!("session round-trip failed: {e}")));
                    }
                }
            }
        }
        unreachable!("the loop returns on both attempts")
    }

    /// Launch `dm-elevated serve-session` elevated (one UAC), signature-pinned, and wait for its
    /// pipe to come up. Returns the pipe name, or `Declined` on UAC cancel.
    fn launch(&self) -> PortResult<Launch> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        // A per-launch pipe leaf. Uniqueness (not secrecy) is what this needs — the server's
        // per-connection client-PID check is the identity gate, not the name. Only `[A-Za-z0-9._-]`
        // so it passes the server's `is_safe_pipe_name`.
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let pipe = format!("dm-elev-{}-{}-{}", self.client_pid, nanos, n);
        let params = format!(
            "serve-session --pipe {} --client-pid {}",
            crate::cmdline::quote_arg(&pipe),
            self.client_pid
        );

        // A1/C3: verify + PIN the helper across the launch (no swap between check and exec).
        let _pin = PinnedHelper::open_verified(&self.helper_path)?;
        let verb = HSTRING::from("runas");
        let file = HSTRING::from(self.helper_path.as_os_str());
        let parameters = HSTRING::from(params);
        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS,
            lpVerb: PCWSTR(verb.as_ptr()),
            lpFile: PCWSTR(file.as_ptr()),
            lpParameters: PCWSTR(parameters.as_ptr()),
            nShow: 0, // SW_HIDE — a background server; UAC still prompts.
            ..Default::default()
        };
        // SAFETY: `info` and its wide-string buffers outlive the call.
        if let Err(e) = unsafe { ShellExecuteExW(&mut info) } {
            return if e.code() == windows::core::HRESULT::from_win32(ERROR_CANCELLED.0) {
                Ok(Launch::Declined)
            } else {
                Err(PortError::Com(format!("ShellExecuteEx runas serve-session failed: {e}")))
            };
        }
        // The server creates the pipe shortly after launch; wait (bounded) for it to appear.
        wait_for_pipe(&pipe)?;
        Ok(Launch::Started(pipe))
    }
}

enum Launch {
    Started(String),
    Declined,
}

/// Poll for the server's pipe to exist, up to ~5s (WaitNamedPipe returns as soon as an instance is
/// available). An establish failure here → the caller falls back to per-op `runas`.
fn wait_for_pipe(pipe: &str) -> PortResult<()> {
    let full = HSTRING::from(format!(r"\\.\pipe\{pipe}"));
    for _ in 0..50 {
        // 100ms per wait; returns TRUE as soon as an instance is ready.
        if unsafe { WaitNamedPipeW(&full, 100) }.as_bool() {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    Err(PortError::Com("the elevated session pipe never came up".into()))
}

/// One request/response round-trip over the pipe.
fn round_trip(pipe: &str, argv: &[&str]) -> Result<(u8, String), String> {
    let full = HSTRING::from(format!(r"\\.\pipe\{pipe}"));
    // Connect with a short retry: the single-threaded server has a brief window between finishing
    // one request (Disconnect+Close) and creating the next instance where CreateFile finds no
    // listener. Retrying (with WaitNamedPipe when it reports BUSY) rides over that window so a
    // healthy session is never mistaken for a dead one — which would otherwise trigger a spurious
    // relaunch + extra UAC (the exact thing this feature removes). ~2s ceiling, then give up.
    let handle = {
        let mut last = String::new();
        let mut got = None;
        for _ in 0..40 {
            match unsafe {
                CreateFileW(
                    &full,
                    (GENERIC_READ.0 | GENERIC_WRITE.0) as u32,
                    FILE_SHARE_MODE(0),
                    None,
                    OPEN_EXISTING,
                    FILE_FLAGS_AND_ATTRIBUTES(0),
                    None,
                )
            } {
                Ok(h) => {
                    got = Some(h);
                    break;
                }
                Err(e) => {
                    last = e.to_string();
                    // Wait briefly for an instance to (re)appear, then retry.
                    let _ = unsafe { WaitNamedPipeW(&full, 50) };
                    std::thread::sleep(std::time::Duration::from_millis(25));
                }
            }
        }
        got.ok_or_else(|| format!("open session pipe: {last}"))?
    };
    // Own the handle via a File so it closes on drop (this connection is one-shot).
    let mut io = unsafe { std::fs::File::from_raw_handle(handle.0 as *mut _) };

    // Request: argc, then each arg length-prefixed.
    write_u32(&mut io, argv.len() as u32)?;
    for a in argv {
        let b = a.as_bytes();
        write_u32(&mut io, b.len() as u32)?;
        io.write_all(b).map_err(|e| format!("write arg: {e}"))?;
    }
    io.flush().map_err(|e| format!("flush request: {e}"))?;

    // Response: code, msg-len, msg.
    let code = read_u32(&mut io)?;
    let len = read_u32(&mut io)?;
    if len > 64 * 1024 {
        return Err(format!("response message too large: {len}"));
    }
    let mut msg = vec![0u8; len as usize];
    io.read_exact(&mut msg).map_err(|e| format!("read response: {e}"))?;
    Ok((code as u8, String::from_utf8_lossy(&msg).into_owned()))
}

use std::os::windows::io::FromRawHandle;

fn write_u32(io: &mut impl Write, v: u32) -> Result<(), String> {
    io.write_all(&v.to_le_bytes()).map_err(|e| format!("write u32: {e}"))
}
fn read_u32(io: &mut impl Read) -> Result<u32, String> {
    let mut b = [0u8; 4];
    io.read_exact(&mut b).map_err(|e| format!("read u32: {e}"))?;
    Ok(u32::from_le_bytes(b))
}
