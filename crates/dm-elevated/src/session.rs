//! The session-scoped elevated server (owner 2026-07-17: "authorize once per app launch").
//!
//! Instead of one `runas`/UAC per privileged operation, the unelevated app launches the helper
//! ONCE in this mode (one UAC), and thereafter drives every privileged verb over a named pipe with
//! no further prompts — until the app exits, when this server shuts itself down (never a lingering
//! elevated process).
//!
//! ## Security model
//!
//! This is a persistent HIGH-integrity process taking commands from a MEDIUM-integrity peer, so the
//! channel is the attack surface. Three independent gates:
//!
//! 1. **Command grammar** — every request's argv is re-parsed through the SAME [`crate::args`]
//!    privilege-boundary grammar the CLI uses, and dispatched to the SAME validated handlers
//!    ([`crate::overlay`], [`crate::desktop_items`]). The pipe therefore grants NO capability the
//!    per-op CLI did not: only overlay apply/restore (validated ICO) and desktop-items apply/restore
//!    (targets re-validated under the Public-desktop/ProgramData roots). `serve-session` itself is
//!    refused over the pipe — no nested servers.
//! 2. **Client identity** — the pipe's DACL admits only the current user, and on EVERY connection
//!    the server checks `GetNamedPipeClientProcessId` equals the exact `client_pid` the launcher
//!    passed. A different same-user process (which could otherwise reach the pipe) is rejected before
//!    a byte is read.
//! 3. **Lifetime** — a watcher thread waits on the client process handle and force-exits this server
//!    the instant the app dies, so consent never outlives the app that obtained it.
//!
//! The wire framing is dependency-free little-endian length-prefixing (no serde in the privileged
//! binary): request = `u32 argc` then `argc × (u32 len, len bytes UTF-8)`; response = `u32 exit_code,
//! u32 msg_len, msg_len bytes UTF-8`.

#[cfg(windows)]
pub use imp::run_serve_session;

/// Non-Windows builds exist only for the host `cargo test` cross-check; the server never runs there.
#[cfg(not(windows))]
pub fn run_serve_session(_pipe: &str, _client_pid: u32) -> Result<(), String> {
    Err("serve-session is only available on Windows".into())
}

/// The largest single request the server will read (argv + all arg bytes). A real command is a few
/// hundred bytes (a verb + a manifest path); this bounds a hostile/framed-wrong peer.
pub const MAX_REQUEST_BYTES: u32 = 64 * 1024;
/// The largest argv count a request may carry (the widest real verb is 5 tokens).
pub const MAX_ARGC: u32 = 16;

/// Dispatch one already-parsed argv to the whitelisted handlers, returning `(exit_code, message)`
/// with the SAME classification the CLI uses. Pure over the [`crate::args`] grammar + the handler
/// entry points, so it is host-testable without a pipe. `serve-session` / unknown / bare verbs are
/// refused — the pipe is not a way to widen the verb set.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn dispatch(argv: &[String]) -> (u8, String) {
    use crate::args::Command;
    match crate::args::parse(argv) {
        Command::ApplyOverlay { .. } | Command::RestoreOverlay => run_overlay(argv),
        Command::ApplyDesktopItems { manifest } => {
            classify_dt(crate::desktop_items::run_apply_file(&manifest))
        }
        Command::RestoreDesktopItems { manifest } => {
            classify_dt(crate::desktop_items::run_restore_file(&manifest))
        }
        // A nested serve-session, Version/None, or anything Unknown is refused over the pipe.
        Command::ServeSession { .. } => (2, "serve-session is not permitted over the session pipe".into()),
        Command::Version | Command::None => (2, "not a privileged verb".into()),
        Command::Unknown(why) => (2, format!("rejected: {why}")),
    }
}

/// The overlay verbs go through the same `overlay` module the CLI `main` uses; kept behind a thin
/// wrapper so [`dispatch`] stays platform-neutral for host tests (the real overlay is Windows-only).
#[cfg(windows)]
fn run_overlay(argv: &[String]) -> (u8, String) {
    use crate::args::Command;
    match crate::args::parse(argv) {
        Command::ApplyOverlay { style, file } => match crate::overlay::apply(style, file.as_deref()) {
            Ok(()) => (0, "ok".into()),
            Err(e) => (3, e),
        },
        Command::RestoreOverlay => match crate::overlay::restore() {
            Ok(()) => (0, "ok".into()),
            Err(e) => (3, e),
        },
        _ => (2, "not an overlay verb".into()),
    }
}
#[cfg(not(windows))]
fn run_overlay(_argv: &[String]) -> (u8, String) {
    (3, "overlay is only available on Windows".into())
}

/// Map a desktop-items batch result to `(exit_code, message)` via the shared failure taxonomy.
#[cfg_attr(not(windows), allow(dead_code))]
fn classify_dt(result: Result<(), String>) -> (u8, String) {
    match result {
        Ok(()) => (0, "ok".into()),
        Err(e) => {
            let code = crate::desktop_items::classify_failure(&e);
            (code, e)
        }
    }
}

#[cfg(windows)]
mod imp {
    use std::io::{Read, Write};
    use std::mem::ManuallyDrop;
    use std::os::windows::io::FromRawHandle;

    use windows::core::{HSTRING, PWSTR};
    use windows::Win32::Foundation::{
        CloseHandle, GetLastError, ERROR_PIPE_CONNECTED, HANDLE, HLOCAL, INVALID_HANDLE_VALUE,
        LocalFree, WAIT_OBJECT_0,
    };
    use windows::Win32::Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
    };
    use windows::Win32::Security::{
        GetTokenInformation, TokenUser, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY,
        TOKEN_USER,
    };
    use windows::Win32::Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX};
    use windows::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, GetNamedPipeClientProcessId,
        PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES,
        PIPE_WAIT,
    };
    use windows::Win32::System::Threading::{
        GetCurrentProcess, OpenProcess, OpenProcessToken, WaitForSingleObject, INFINITE,
        PROCESS_SYNCHRONIZE,
    };

    use super::{MAX_ARGC, MAX_REQUEST_BYTES};

    /// Serve until the launching app (`client_pid`) exits. Every error before the loop is fatal
    /// (the caller maps it to a non-zero exit); inside the loop a per-connection error is logged to
    /// stderr and the loop continues (one bad/rejected client must not kill the session).
    pub fn run_serve_session(pipe: &str, client_pid: u32) -> Result<(), String> {
        let full = format!(r"\\.\pipe\{pipe}");
        // The client process handle (SYNCHRONIZE only — we never touch its memory). Opening it also
        // proves the pid currently names a live process we can wait on.
        let client = open_client(client_pid)?;
        // Watcher: the instant the app exits, force this elevated server down. A dedicated thread so
        // a blocked ConnectNamedPipe can never outlive the app.
        spawn_death_watch(client);

        // Security descriptor: DACL = LocalSystem + the current user only; SACL = a MEDIUM
        // mandatory label so the medium-integrity app can open the pipe a high-integrity process
        // created (without it the default high-IL label blocks the peer). Built ONCE and reused.
        let sd = SecurityDescriptor::current_user_medium()?;

        let mut first = true;
        loop {
            let handle = create_instance(&full, &sd, first)?;
            first = false;
            let served = serve_one(handle, client_pid);
            // Always tear the instance down before the next accept.
            unsafe {
                let _ = DisconnectNamedPipe(handle);
                let _ = CloseHandle(handle);
            }
            if let Err(e) = served {
                eprintln!("session: connection error: {e}");
            }
        }
    }

    /// Accept one connection, verify the client pid, process exactly one request, reply. The pipe
    /// HANDLE stays owned by the caller (Disconnect/Close there); I/O borrows it through a
    /// `ManuallyDrop<File>` that never closes it.
    fn serve_one(pipe: HANDLE, client_pid: u32) -> Result<(), String> {
        // ConnectNamedPipe blocks until a client connects; the death-watch thread force-exits the
        // process if the app dies first, so this never strands the server.
        let connected = unsafe { ConnectNamedPipe(pipe, None) };
        if connected.is_err() {
            // ERROR_PIPE_CONNECTED = a client connected between Create and Connect — that is success.
            let code = unsafe { GetLastError() };
            if code != ERROR_PIPE_CONNECTED {
                return Err(format!("ConnectNamedPipe failed: {code:?}"));
            }
        }
        // IDENTITY GATE: only the exact launching app process may drive us.
        let mut pid = 0u32;
        unsafe { GetNamedPipeClientProcessId(pipe, &mut pid) }
            .map_err(|e| format!("GetNamedPipeClientProcessId failed: {e}"))?;
        if pid != client_pid {
            return Err(format!("rejected connection from pid {pid} (expected {client_pid})"));
        }
        // Borrow the pipe for buffered framing WITHOUT taking ownership of the handle (ManuallyDrop
        // never closes it; the caller Disconnects/Closes). Deref to the inner `File`, which is what
        // implements Read/Write.
        let mut io = ManuallyDrop::new(unsafe { std::fs::File::from_raw_handle(pipe.0 as *mut _) });
        let file: &mut std::fs::File = &mut io;
        let argv = read_request(file)?;
        let (code, msg) = super::dispatch(&argv);
        write_response(file, code, &msg)
    }

    /// Read one length-prefixed request into an argv, bounding both the count and the total bytes.
    fn read_request(io: &mut impl Read) -> Result<Vec<String>, String> {
        let argc = read_u32(io)?;
        if argc > MAX_ARGC {
            return Err(format!("argc {argc} exceeds the {MAX_ARGC} cap"));
        }
        let mut total = 0u32;
        let mut argv = Vec::with_capacity(argc as usize);
        for _ in 0..argc {
            let len = read_u32(io)?;
            total = total.checked_add(len).ok_or("request length overflow")?;
            if total > MAX_REQUEST_BYTES {
                return Err(format!("request exceeds the {MAX_REQUEST_BYTES}-byte cap"));
            }
            let mut buf = vec![0u8; len as usize];
            io.read_exact(&mut buf).map_err(|e| format!("read request arg: {e}"))?;
            argv.push(String::from_utf8(buf).map_err(|_| "request arg is not UTF-8".to_string())?);
        }
        Ok(argv)
    }

    fn write_response(io: &mut impl Write, code: u8, msg: &str) -> Result<(), String> {
        write_u32(io, code as u32)?;
        let bytes = msg.as_bytes();
        write_u32(io, bytes.len() as u32)?;
        io.write_all(bytes).map_err(|e| format!("write response body: {e}"))?;
        io.flush().map_err(|e| format!("flush response: {e}"))
    }

    fn read_u32(io: &mut impl Read) -> Result<u32, String> {
        let mut b = [0u8; 4];
        io.read_exact(&mut b).map_err(|e| format!("read u32: {e}"))?;
        Ok(u32::from_le_bytes(b))
    }
    fn write_u32(io: &mut impl Write, v: u32) -> Result<(), String> {
        io.write_all(&v.to_le_bytes()).map_err(|e| format!("write u32: {e}"))
    }

    /// Create one pipe instance. The FIRST instance carries `FILE_FLAG_FIRST_PIPE_INSTANCE` so a
    /// pre-squatting process that already owns this name makes us FAIL CLOSED (rather than silently
    /// sharing the namespace with an impostor). Byte-mode, local clients only, blocking.
    fn create_instance(full: &str, sd: &SecurityDescriptor, first: bool) -> Result<HANDLE, String> {
        let mut sa = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: sd.0 .0,
            bInheritHandle: false.into(),
        };
        let mut open_mode = PIPE_ACCESS_DUPLEX;
        if first {
            open_mode |= FILE_FLAG_FIRST_PIPE_INSTANCE;
        }
        let handle = unsafe {
            CreateNamedPipeW(
                &HSTRING::from(full),
                open_mode,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_UNLIMITED_INSTANCES,
                4096,
                4096,
                0,
                Some(&mut sa),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(format!("CreateNamedPipeW failed: {:?}", unsafe { GetLastError() }));
        }
        Ok(handle)
    }

    /// Open the launching app for lifetime-watching. SYNCHRONIZE is the ONLY right we request — we
    /// wait on it, never read its memory. A failure here means the pid is already gone/invalid.
    fn open_client(pid: u32) -> Result<HANDLE, String> {
        let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) }
            .map_err(|e| format!("OpenProcess({pid}) failed: {e}"))?;
        Ok(handle)
    }

    /// A thread that force-exits this server the instant the app dies (so consent never outlives it).
    fn spawn_death_watch(client: HANDLE) {
        let raw = client.0 as isize;
        std::thread::spawn(move || {
            let h = HANDLE(raw as *mut _);
            unsafe {
                if WaitForSingleObject(h, INFINITE) == WAIT_OBJECT_0 {
                    // The app exited — tear the whole elevated process down.
                    std::process::exit(0);
                }
            }
        });
    }

    /// An owned `PSECURITY_DESCRIPTOR` (LocalFree on drop) plus a live handle for FFI.
    struct SecurityDescriptor(PSECURITY_DESCRIPTOR);
    impl SecurityDescriptor {
        /// DACL: LocalSystem (`SY`) + the current user, generic-all; SACL: MEDIUM mandatory label,
        /// no-write-up. The medium label is what lets the unelevated (medium-IL) app open a pipe
        /// this high-IL process created; the user-only DACL keeps other users out; the per-request
        /// pid check keeps other same-user processes out.
        fn current_user_medium() -> Result<Self, String> {
            let sid = current_user_sid_string()?;
            // SDDL: Owner = user; DACL grants SYSTEM + the user generic-all; SACL sets a medium
            // integrity label with no-write-up (blocks lower-IL subjects).
            let sddl = format!("O:{sid}D:(A;;GA;;;SY)(A;;GA;;;{sid})S:(ML;;NW;;;ME)");
            let mut psd = PSECURITY_DESCRIPTOR::default();
            unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    &HSTRING::from(sddl),
                    SDDL_REVISION_1,
                    &mut psd,
                    None,
                )
            }
            .map_err(|e| format!("building the pipe security descriptor failed: {e}"))?;
            Ok(SecurityDescriptor(psd))
        }
    }
    impl Drop for SecurityDescriptor {
        fn drop(&mut self) {
            if !self.0 .0.is_null() {
                unsafe { let _ = LocalFree(Some(HLOCAL(self.0 .0))); }
            }
        }
    }

    /// The current process user's SID as an SDDL string (e.g. `S-1-5-21-...`).
    fn current_user_sid_string() -> Result<String, String> {
        unsafe {
            let mut token = HANDLE::default();
            OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)
                .map_err(|e| format!("OpenProcessToken failed: {e}"))?;
            let _guard = HandleGuard(token);
            // Size probe, then read the TOKEN_USER.
            let mut len = 0u32;
            let _ = GetTokenInformation(token, TokenUser, None, 0, &mut len);
            if len == 0 {
                return Err("GetTokenInformation(TokenUser) size probe failed".into());
            }
            let mut buf = vec![0u8; len as usize];
            GetTokenInformation(token, TokenUser, Some(buf.as_mut_ptr() as *mut _), len, &mut len)
                .map_err(|e| format!("GetTokenInformation(TokenUser) failed: {e}"))?;
            let tu = &*(buf.as_ptr() as *const TOKEN_USER);
            let mut pstr = PWSTR::null();
            ConvertSidToStringSidW(tu.User.Sid, &mut pstr)
                .map_err(|e| format!("ConvertSidToStringSidW failed: {e}"))?;
            let s = pstr.to_string().map_err(|e| e.to_string())?;
            let _ = LocalFree(Some(HLOCAL(pstr.0 as *mut _)));
            Ok(s)
        }
    }

    /// Closes a HANDLE on drop (for the token).
    struct HandleGuard(HANDLE);
    impl Drop for HandleGuard {
        fn drop(&mut self) {
            if !self.0.is_invalid() {
                unsafe { let _ = CloseHandle(self.0); }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_refuses_non_privileged_and_nested_verbs() {
        // A nested serve-session over the pipe is refused (no server spawns another server).
        let (code, _) = dispatch(&["serve-session".into(), "--pipe".into(), "x".into(), "--client-pid".into(), "5".into()]);
        assert_eq!(code, 2);
        // Bare/unknown verbs are refused with exit 2 (never silently run).
        assert_eq!(dispatch(&["version".into()]).0, 2);
        assert_eq!(dispatch(&["do-evil".into()]).0, 2);
        assert_eq!(dispatch(&[]).0, 2);
        // A desktop-items verb with a missing manifest is a grammar rejection (exit 2), not a run.
        assert_eq!(dispatch(&["apply-desktop-items".into()]).0, 2);
    }
}
