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
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use dm_domain::{PortError, PortResult};
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::{
    ERROR_CANCELLED, FILETIME, GENERIC_READ, GENERIC_WRITE, HANDLE, WAIT_TIMEOUT,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_MODE, OPEN_EXISTING,
};
use windows::Win32::System::Pipes::{GetNamedPipeServerProcessId, WaitNamedPipeW};
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentProcessId, GetProcessId, GetProcessTimes, WaitForSingleObject,
};
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

/// A live elevated session: the server's pipe name, its PID, and an OWNED handle to the elevated
/// helper process. The handle is closed on drop (no leak — codex P2c) and, crucially, lets each
/// round-trip confirm the helper is STILL ALIVE (its handle not signaled) before trusting the pipe's
/// server pid — a live process's pid is never reused, so alive + pid-match == our helper. Holding the
/// handle does NOT by itself prevent pid reuse (a terminated pid is reusable), so the aliveness check
/// is what makes the server-pid authentication sound (codex 2026-07-17 High).
struct Session {
    pipe: String,
    server_pid: u32,
    server: OwnedHandle,
}

/// A lazily-launched, session-lived elevated helper reached over a named pipe.
pub struct SessionElevated {
    helper_path: PathBuf,
    client_pid: u32,
    /// This process's creation time (FILETIME packed to u64). Passed to the server as the second half
    /// of our identity so it can reject a pid-reuse impostor (codex P1).
    client_created: u64,
    /// The live session, or `None` before first launch / after the helper died.
    active: Mutex<Option<Session>>,
}

impl SessionElevated {
    pub fn new(helper_path: PathBuf) -> Self {
        Self {
            helper_path,
            client_pid: unsafe { GetCurrentProcessId() },
            client_created: own_creation_time(),
            active: Mutex::new(None),
        }
    }

    /// Send one framed argv to the elevated session, launching it (one UAC) if it is not up.
    /// `Ok(SessionSend)` = the request reached the helper (or the user declined the launch);
    /// `Err` = the session could NOT be established — the caller should fall back to per-op `runas`.
    pub fn send(&self, argv: &[&str]) -> PortResult<SessionSend> {
        // ENABLED after a three-round codex security review APPROVED the hardening (2026-07-17): P1
        // (identity is the server-verified (pid, creation-time) pair, RE-checked per connection), P2
        // (client authenticates the pipe server's pid == the helper it launched, with a post-connect
        // fail-closed aliveness re-check), P2b (DACL from the CLIENT's SID), P2c (helper handle owned,
        // not leaked). The channel remains a pure OPTIMISATION: any establish/round-trip failure falls
        // back to per-op `runas` (see the caller), so it can only ever REDUCE prompts, never break
        // elevation. Set DESKMAKEOVER_SESSION_ELEVATION_OFF=1 to force the per-op path (a kill switch).
        if std::env::var_os("DESKMAKEOVER_SESSION_ELEVATION_OFF").is_some() {
            return Err(PortError::Com("session elevation disabled by DESKMAKEOVER_SESSION_ELEVATION_OFF".into()));
        }
        let mut active = self.active.lock().unwrap();
        for attempt in 0..2u8 {
            // Ensure a live session.
            if active.is_none() {
                match self.launch()? {
                    Launch::Started(session) => *active = Some(session),
                    Launch::Declined => return Ok(SessionSend::Declined),
                }
            }
            let session = active.as_ref().expect("just ensured");
            // `round_trip` authenticates the server AFTER connecting (server-pid match + a fail-closed
            // aliveness re-check on `session.server`, together sound against pid reuse — codex High).
            match round_trip(&session.pipe, session.server_pid, &session.server, argv) {
                Ok((code, message)) => return Ok(SessionSend::Ran { code, message }),
                // The helper died / the pipe broke / an impostor answered. Drop the dead session and,
                // on the first attempt, relaunch (a fresh UAC). A second failure is a real establish
                // failure → fall back to per-op runas.
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
            "serve-session --pipe {} --client-pid {} --client-created {}",
            crate::cmdline::quote_arg(&pipe),
            self.client_pid,
            self.client_created
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
        // SEE_MASK_NOCLOSEPROCESS populated `info.hProcess` with the elevated helper. Read its pid
        // (for the server-identity check in `round_trip`), then OWN the handle so it is closed on drop
        // (no leak — codex P2c) and each round-trip can re-confirm the helper is still ALIVE before
        // trusting the pipe's server pid (a live process's pid is never reused). Holding the handle
        // does NOT by itself prevent pid reuse — the aliveness re-check does (codex 2026-07-17 High).
        let server_pid = unsafe { GetProcessId(info.hProcess) };
        let server: OwnedHandle = unsafe { OwnedHandle::from_raw_handle(info.hProcess.0 as *mut _) };
        if server_pid == 0 {
            // `server` drops here, closing the handle. Without a verifiable server pid we cannot
            // authenticate the pipe (codex P2), so refuse the session and fall back to per-op runas.
            return Err(PortError::Com(
                "could not read the elevated helper's process id (cannot authenticate the pipe server)".into(),
            ));
        }
        // The server creates the pipe shortly after launch; wait (bounded) for it to appear.
        wait_for_pipe(&pipe)?;
        Ok(Launch::Started(Session { pipe, server_pid, server }))
    }
}

/// Whether the elevated helper is DEFINITELY still alive. FAIL-CLOSED: only `WAIT_TIMEOUT` (the wait
/// timed out because the process handle is not signaled → still running) returns true. A signaled
/// handle (exited), `WAIT_FAILED`, or any other result returns false, so a dead/uncertain helper —
/// whose pid could be recycled to an impostor — is never trusted (codex 2026-07-17 High + fail-closed).
fn helper_alive(server: &OwnedHandle) -> bool {
    // SAFETY: borrows the owned handle; a 0-ms wait polls the signaled state without blocking.
    unsafe { WaitForSingleObject(HANDLE(server.as_raw_handle() as *mut _), 0) == WAIT_TIMEOUT }
}

/// This process's creation time as a u64 (FILETIME high/low packed), for the session-identity pair.
fn own_creation_time() -> u64 {
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    // SAFETY: GetCurrentProcess is a pseudo-handle; all four out-params are owned locals.
    if unsafe {
        GetProcessTimes(GetCurrentProcess(), &mut creation, &mut exit, &mut kernel, &mut user)
    }
    .is_err()
    {
        return 0;
    }
    ((creation.dwHighDateTime as u64) << 32) | (creation.dwLowDateTime as u64)
}

enum Launch {
    Started(Session),
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

/// One request/response round-trip over the pipe. `expected_server_pid` + `server` (the OWNED handle
/// to the elevated helper we launched) authenticate the peer: the pipe's server pid must equal
/// `expected_server_pid` AND the helper handle must still be UNSIGNALED at that moment (a live pid is
/// never reused), both checked on the ALREADY-CONNECTED pipe before a request byte is written (codex
/// 2026-07-17 High: a pre-connect aliveness check races the ~2s connect window).
fn round_trip(
    pipe: &str,
    expected_server_pid: u32,
    server: &OwnedHandle,
    argv: &[&str],
) -> Result<(u8, String), String> {
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
    // Own the handle via a File so it closes on drop (this connection is one-shot). ANY early return
    // below drops `io` and closes the handle — including the impostor rejection next.
    let mut io = unsafe { std::fs::File::from_raw_handle(handle.0 as *mut _) };

    // P2 SERVER AUTHENTICATION: before writing a single request byte, verify the pipe's server pid is
    // the exact elevated helper we launched. A same-user process that squatted this pipe name (racing
    // our CreateNamedPipe, which then fails FIRST_PIPE_INSTANCE-closed) would otherwise receive our
    // request and forge an "Applied" reply — masking that the privileged op never ran, and leaking the
    // verb + manifest path. `GetNamedPipeServerProcessId` is unforgeable by the peer (codex P2).
    let mut server_pid = 0u32;
    unsafe { GetNamedPipeServerProcessId(HANDLE(io.as_raw_handle() as *mut _), &mut server_pid) }
        .map_err(|e| format!("GetNamedPipeServerProcessId failed: {e}"))?;
    if server_pid != expected_server_pid {
        return Err(format!(
            "pipe server pid {server_pid} != our helper {expected_server_pid} — refusing an impostor"
        ));
    }
    // Fail-closed aliveness re-check ON THE CONNECTED endpoint: the server pid just matched, but if
    // the real helper exited during the connect window its pid could have been recycled to the
    // impostor that answered. Only `WAIT_TIMEOUT` (helper still running → its pid cannot belong to
    // anyone else) may proceed; signaled / WAIT_FAILED / any other result refuses (codex High + the
    // fail-closed Low). Once past this on a live handle, the connected endpoint cannot be swapped.
    if !helper_alive(server) {
        return Err("the elevated helper is no longer alive — refusing a possibly-recycled server".into());
    }

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

fn write_u32(io: &mut impl Write, v: u32) -> Result<(), String> {
    io.write_all(&v.to_le_bytes()).map_err(|e| format!("write u32: {e}"))
}
fn read_u32(io: &mut impl Read) -> Result<u32, String> {
    let mut b = [0u8; 4];
    io.read_exact(&mut b).map_err(|e| format!("read u32: {e}"))?;
    Ok(u32::from_le_bytes(b))
}
