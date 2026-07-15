//! The privileged overlay verbs (ADR-0021 §4): apply/restore the machine-wide
//! `HKLM ...\Shell Icons\29` shortcut-overlay value. Ported from
//! `ElevatedHelper/OverlayCommands.cs`.
//!
//! Invariants preserved from the oracle:
//! * the pre-DeskMakeover value is snapshotted exactly once, DURABLY, before the first
//!   modification, under `%ProgramData%`, so restore never depends on the caller;
//! * the registry NEVER points at a caller-supplied path — the ICO is validated and copied into
//!   `%ProgramData%` first (LPE guard);
//! * restore rewrites the exact original value (or deletes it when it was absent), zero residue.
//!
//! Hardened beyond the oracle (codex R2-#2/#3): the snapshot records the original value's TYPE +
//! raw bytes (get_raw_value/set_raw_value), so a REG_EXPAND_SZ / non-string / embedded-NUL original
//! is restored verbatim instead of being lost; and restore CAS-checks the live value against the one
//! we installed, refusing to clobber a third-party change made after our apply.

use crate::args::Style;

/// Applies the overlay for `style`, using the rendered ICO at `source_file`.
pub fn apply(style: Style, source_file: Option<&str>) -> Result<(), String> {
    #[cfg(windows)]
    {
        windows_impl::apply(style, source_file)
    }
    #[cfg(not(windows))]
    {
        let _ = (style, source_file);
        Err("overlay verbs only run on Windows".to_string())
    }
}

/// Restores the original overlay registry state.
pub fn restore() -> Result<(), String> {
    #[cfg(windows)]
    {
        windows_impl::restore()
    }
    #[cfg(not(windows))]
    {
        Err("overlay verbs only run on Windows".to_string())
    }
}

#[cfg(windows)]
mod windows_impl {
    use std::path::{Path, PathBuf};

    use std::fmt::Write as _;

    use winreg::enums::*;
    use winreg::{RegKey, RegValue};

    use crate::args::Style;
    use crate::guards::read_capped_ico;
    use crate::secure_dir::secure_data_dir;

    const SHELL_ICONS_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\Shell Icons";
    const OVERLAY_VALUE: &str = "29";
    const ABSENT_MARKER: &str = "__absent__";
    const STATE_FILE: &str = "overlay-state.txt";

    pub fn apply(style: Style, source_file: Option<&str>) -> Result<(), String> {
        // The data dir is resolved from a known folder and proven admin-owned + non-reparse with a
        // restrictive DACL before we trust anything inside it (P1 LPE fix — see secure_dir).
        let dir = secure_data_dir()?;
        let ico = materialize_ico(&dir, style, source_file)?;

        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let key = hklm.create_subkey(SHELL_ICONS_KEY).map_err(io)?.0;
        // Point the registry at the ProgramData copy — NEVER the caller path (LPE guard).
        let installed = format!("{},0", ico.display());

        // Snapshot the pre-DeskMakeover value exactly once, DURABLY, BEFORE touching the registry.
        // The write must be fsync'd first (codex 2026-07-12): a plain `fs::write` that has not
        // reached disk when the registry `set_value` below lands means a power loss loses the
        // snapshot, and the NEXT apply then re-captures OUR OWN value as the "original" — a permanent
        // loss of the true restore anchor. The state file lives in the hardened, admin-only data dir,
        // so a standard user cannot pre-plant it to defeat this guard.
        let state = dir.join(STATE_FILE);
        if !state.exists() {
            // Capture the original value WITH ITS TYPE via get_raw_value (codex R2-#2): the old lossy
            // `get_value::<String>` recorded a REG_EXPAND_SZ / non-string / embedded-NUL original as
            // "__absent__", so restore DELETED another tool's real value. `None` is a genuine absence;
            // a NotFound is absent; any other read fault propagates (fail closed, never guess absent).
            let original = match key.get_raw_value(OVERLAY_VALUE) {
                Ok(v) => Some(v),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(io(e)),
            };
            // Record BOTH the raw original and the value we are about to install, so restore can CAS
            // the live value against ours and refuse to clobber a third-party change (codex R2-#3).
            // We know `installed` before the set, so the snapshot still precedes the registry write.
            // Non-replacing atomic claim: if another elevated apply raced in between the check above
            // and here, our publish fails-closed and does NOT overwrite their real-original snapshot.
            let body = serialize_state(
                original.as_ref().map(|v| (v.vtype.clone() as u32, v.bytes.as_slice())),
                &installed,
            );
            snapshot_once(&dir, STATE_FILE, body.as_bytes())?;
        }

        key.set_value(OVERLAY_VALUE, &installed).map_err(io)?;
        notify_shell();
        Ok(())
    }

    pub fn restore() -> Result<(), String> {
        let dir = secure_data_dir()?;
        let state = dir.join(STATE_FILE);
        if !state.exists() {
            return Ok(()); // untouched — nothing to restore
        }
        let raw = read_state_capped(&state)?;
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let key = hklm.create_subkey(SHELL_ICONS_KEY).map_err(io)?.0;

        match parse_state(&raw) {
            Some(st) => {
                // CAS (codex R2-#3): only restore if the LIVE value is still the one WE installed. If a
                // third-party tool changed Shell Icons\29 after our apply, refuse and KEEP the snapshot
                // so the user can resolve it — never silently clobber their newer setting. Compared as a
                // string via winreg's own get/set round-trip, so encoding details stay symmetric.
                let current: Option<String> = key.get_value(OVERLAY_VALUE).ok();
                if current.as_deref() != Some(st.installed.as_str()) {
                    return Err(
                        "the shortcut-overlay value changed since DeskMakeover set it (another app owns \
                         it now); refusing to restore over it — clear it manually or re-apply to take \
                         ownership"
                            .to_string(),
                    );
                }
                restore_original(&key, st.original.as_ref())?;
            }
            // Legacy v1 state (a bare REG_SZ string / "__absent__" from before the raw+CAS format):
            // restore with the old semantics so an item styled by an older build still reverts.
            None => restore_v1(&key, raw.trim())?,
        }

        std::fs::remove_file(&state).map_err(io)?;
        notify_shell();
        Ok(())
    }

    /// The parsed v2 overlay state: the raw original value (type + bytes; `None` = it was absent) and
    /// the exact string DeskMakeover installed (the CAS anchor).
    struct OverlayState {
        original: Option<(u32, Vec<u8>)>,
        installed: String,
    }

    const STATE_MAGIC: &str = "DMOVL2";

    /// Serializes the v2 state as all-ASCII text (hex-encoded bytes) so it round-trips through the
    /// UTF-8 `read_state_capped` reader even when the original value is UTF-16/binary.
    fn serialize_state(original: Option<(u32, &[u8])>, installed: &str) -> String {
        let orig_line = match original {
            None => "orig:A".to_string(),
            Some((vtype, bytes)) => format!("orig:P:{vtype}:{}", to_hex(bytes)),
        };
        format!("{STATE_MAGIC}\n{orig_line}\ninstalled:{}\n", to_hex(installed.as_bytes()))
    }

    /// Parses the v2 state. Returns `None` for anything that is not v2 (an old v1 file), so the caller
    /// can fall back to the legacy restore path.
    fn parse_state(s: &str) -> Option<OverlayState> {
        let mut lines = s.lines();
        if lines.next()? != STATE_MAGIC {
            return None;
        }
        let orig_line = lines.next()?;
        let installed_hex = lines.next()?.strip_prefix("installed:")?;
        let original = if orig_line == "orig:A" {
            None
        } else {
            let (vtype_str, hex) = orig_line.strip_prefix("orig:P:")?.split_once(':')?;
            Some((vtype_str.parse::<u32>().ok()?, from_hex(hex)?))
        };
        let installed = String::from_utf8(from_hex(installed_hex)?).ok()?;
        Some(OverlayState { original, installed })
    }

    /// Restores the raw original value exactly (preserving its type) or deletes ours when the original
    /// was absent. A genuine NotFound on delete is idempotent; any other fault propagates (audit F6).
    fn restore_original(key: &RegKey, original: Option<&(u32, Vec<u8>)>) -> Result<(), String> {
        match original {
            None => match key.delete_value(OVERLAY_VALUE) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(io(e)),
            },
            Some((vtype, bytes)) => {
                let value = RegValue { bytes: bytes.clone(), vtype: regtype_from_u32(*vtype) };
                key.set_raw_value(OVERLAY_VALUE, &value).map_err(io)
            }
        }
    }

    /// Legacy v1 restore: the state file held the original REG_SZ string, or the absent marker.
    fn restore_v1(key: &RegKey, original: &str) -> Result<(), String> {
        if original == ABSENT_MARKER {
            match key.delete_value(OVERLAY_VALUE) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(io(e)),
            }
        } else {
            key.set_value(OVERLAY_VALUE, &original).map_err(io)
        }
    }

    /// Maps a stored registry-type discriminant back to a `RegType`. An unknown/newer type falls back
    /// to `REG_BINARY`, which set_raw_value writes verbatim — so the original bytes are still restored.
    fn regtype_from_u32(v: u32) -> RegType {
        match v {
            x if x == REG_NONE as u32 => REG_NONE,
            x if x == REG_SZ as u32 => REG_SZ,
            x if x == REG_EXPAND_SZ as u32 => REG_EXPAND_SZ,
            x if x == REG_BINARY as u32 => REG_BINARY,
            x if x == REG_DWORD as u32 => REG_DWORD,
            x if x == REG_DWORD_BIG_ENDIAN as u32 => REG_DWORD_BIG_ENDIAN,
            x if x == REG_LINK as u32 => REG_LINK,
            x if x == REG_MULTI_SZ as u32 => REG_MULTI_SZ,
            x if x == REG_RESOURCE_LIST as u32 => REG_RESOURCE_LIST,
            x if x == REG_FULL_RESOURCE_DESCRIPTOR as u32 => REG_FULL_RESOURCE_DESCRIPTOR,
            x if x == REG_RESOURCE_REQUIREMENTS_LIST as u32 => REG_RESOURCE_REQUIREMENTS_LIST,
            x if x == REG_QWORD as u32 => REG_QWORD,
            _ => REG_BINARY,
        }
    }

    fn to_hex(bytes: &[u8]) -> String {
        let mut s = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            let _ = write!(s, "{b:02x}");
        }
        s
    }

    fn from_hex(s: &str) -> Option<Vec<u8>> {
        if s.len() % 2 != 0 {
            return None;
        }
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect()
    }

    /// Validates the rendered ICO and copies it into the hardened data `dir`, returning its path.
    ///
    /// `read_capped_ico` opens the caller's `--file` exactly once and does the size cap and the
    /// structural `parse` validation through that single handle, so a swap of the path between a
    /// check and the read cannot smuggle an over-cap file past the cap (TOCTOU). The exact
    /// validated bytes we already hold are written to ProgramData — no second read of `src`.
    ///
    /// NOTE (blind-write divergence): the oracle generated the built-in refined/transparent ICOs
    /// in-process (`OverlayBadgeIconFactory`). Here EVERY style's ICO arrives as a validated
    /// `--file` from the client, which owns the icon core. The LPE guard (validate + copy into
    /// ProgramData) is applied uniformly. [icon-core-need] the client renders the
    /// transparent/refined overlay ICOs. [WINDOWS-VERIFY] registry + refresh behaviour.
    fn materialize_ico(dir: &Path, style: Style, source_file: Option<&str>) -> Result<PathBuf, String> {
        let src = source_file
            .ok_or_else(|| format!("--file (a rendered .ico) is required for the {} overlay", style.as_str()))?;
        // Reject a UNC/device/relative path SHAPE (audit F6) BEFORE opening it as SYSTEM — a UNC path
        // would authenticate to an attacker's server; a relative one resolves against the cwd.
        crate::guards::validate_overlay_path(src)?;
        let bytes = read_capped_ico(Path::new(src))?;
        // Atomic O_EXCL-temp + fsync + rename, matching the durability discipline the
        // .lnk/.url/desktop.ini writers use: this ICO is referenced live by HKLM Shell Icons and
        // read by Explorer's icon cache, so a crash mid-write must not leave a torn/corrupt icon
        // (ELEV-2), and the temp is create_new so a symlink can't be followed (defense-in-depth atop
        // the admin-only secure_dir).
        let name = format!("{}-overlay.ico", style.as_str());
        write_durable(dir, &name, &bytes)
    }

    /// Writes `bytes` to a UNIQUE `O_EXCL` temp in `dir` (create_new refuses to follow a pre-placed
    /// symlink — defense-in-depth atop the admin-only secure_dir), `FlushFileBuffers`, and returns
    /// the temp path for the caller to publish. A partial temp is cleaned up on write failure.
    fn write_temp(dir: &Path, name: &str, bytes: &[u8]) -> std::io::Result<PathBuf> {
        use std::io::{ErrorKind, Write};
        let pid = std::process::id();
        let (tmp, mut file) = {
            let mut attempt = 0u32;
            loop {
                let tmp = dir.join(format!(".{name}.dm-{pid}-{attempt}.tmp"));
                match std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp) {
                    Ok(f) => break (tmp, f),
                    Err(e) if e.kind() == ErrorKind::AlreadyExists && attempt < 1000 => {
                        attempt += 1;
                    }
                    Err(e) => return Err(e),
                }
            }
        };
        let written: std::io::Result<()> = (|| {
            file.write_all(bytes)?;
            file.sync_all()
        })();
        drop(file);
        if let Err(e) = written {
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
        Ok(tmp)
    }

    /// Durably + atomically publishes `bytes` to `dir/name`, REPLACING any existing file, and returns
    /// its path. The temp is fsync'd, then published with `MoveFileEx(WRITE_THROUGH|REPLACE_EXISTING)`
    /// so the namespace change is durable before returning (the ProgramData ICO, which a re-apply
    /// legitimately rewrites). [WINDOWS-VERIFY] the write-through move on NTFS.
    fn write_durable(dir: &Path, name: &str, bytes: &[u8]) -> Result<PathBuf, String> {
        use windows::core::HSTRING;
        use windows::Win32::Storage::FileSystem::{
            MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
        };
        let tmp = write_temp(dir, name, bytes).map_err(io)?;
        let target = dir.join(name);
        // SAFETY: valid UTF-16 paths; no buffers retained past the call.
        let moved = unsafe {
            MoveFileExW(
                &HSTRING::from(tmp.as_os_str()),
                &HSTRING::from(target.as_os_str()),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        };
        if let Err(e) = moved {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.to_string());
        }
        Ok(target)
    }

    /// Durably + atomically publishes the pre-first-apply snapshot to `dir/name`, NON-replacing: if
    /// the target already exists (a concurrent or prior elevated apply already snapshotted the true
    /// original), the write-through move fails-closed and we DISCARD our copy rather than clobber
    /// theirs with a possibly-already-modified value — closing the cross-process snapshot-once race
    /// (handoff §8a #4). WRITE_THROUGH makes the snapshot durable BEFORE the caller modifies HKLM.
    /// [WINDOWS-VERIFY] the write-through + non-replacing move on NTFS.
    fn snapshot_once(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
        use windows::core::HSTRING;
        use windows::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_WRITE_THROUGH};
        let tmp = write_temp(dir, name, bytes).map_err(io)?;
        let target = dir.join(name);
        // No MOVEFILE_REPLACE_EXISTING: the move fails if the target exists (another process won).
        // SAFETY: valid UTF-16 paths; no buffers retained past the call.
        let moved = unsafe {
            MoveFileExW(
                &HSTRING::from(tmp.as_os_str()),
                &HSTRING::from(target.as_os_str()),
                MOVEFILE_WRITE_THROUGH,
            )
        };
        let _ = std::fs::remove_file(&tmp); // consumed by the move on success; discarded on collision
        match moved {
            Ok(()) => Ok(()),
            // An existing snapshot means another process satisfied snapshot-once — the desired no-op.
            Err(_) if target.exists() => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Bounded read of the snapshot state file (audit F6): it holds a small registry-value snapshot in
    /// the admin-only data dir, so cap the read rather than slurp an arbitrary (corrupt/planted) file.
    const MAX_STATE_BYTES: u64 = 64 * 1024;
    fn read_state_capped(path: &Path) -> Result<String, String> {
        use std::io::Read;
        let f = std::fs::File::open(path).map_err(io)?;
        let mut buf = Vec::new();
        f.take(MAX_STATE_BYTES + 1).read_to_end(&mut buf).map_err(io)?;
        if buf.len() as u64 > MAX_STATE_BYTES {
            return Err("overlay state file exceeds the cap".to_string());
        }
        String::from_utf8(buf).map_err(|e| e.to_string())
    }

    fn notify_shell() {
        use windows::Win32::UI::Shell::{SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_IDLIST};
        // SAFETY: documented global icon-association refresh; takes no ownership.
        unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None) };
    }

    fn io(e: std::io::Error) -> String {
        e.to_string()
    }

    #[cfg(test)]
    mod tests {
        use winreg::enums::*;

        use super::*;

        #[test]
        fn hex_round_trips_arbitrary_bytes() {
            let bytes = [0x00u8, 0x66, 0xFF, 0x2c, 0x30, 0x00];
            assert_eq!(from_hex(&to_hex(&bytes)), Some(bytes.to_vec()));
            assert_eq!(from_hex("zz"), None); // non-hex
            assert_eq!(from_hex("abc"), None); // odd length
        }

        #[test]
        fn state_round_trips_an_absent_original() {
            let body = serialize_state(None, r"C:\ProgramData\dm\refined-overlay.ico,0");
            let st = parse_state(&body).expect("v2 parses");
            assert!(st.original.is_none());
            assert_eq!(st.installed, r"C:\ProgramData\dm\refined-overlay.ico,0");
        }

        #[test]
        fn state_round_trips_a_reg_expand_sz_original_verbatim() {
            // The exact case the old lossy-String snapshot dropped: a REG_EXPAND_SZ value whose
            // UTF-16LE bytes are not valid UTF-8. It must survive serialize→parse byte-for-byte.
            let orig_bytes: Vec<u8> = "%SystemRoot%\\a.ico,0\0".encode_utf16().flat_map(u16::to_le_bytes).collect();
            let body = serialize_state(Some((REG_EXPAND_SZ as u32, &orig_bytes)), "installed,0");
            let st = parse_state(&body).expect("v2 parses");
            assert_eq!(st.original, Some((REG_EXPAND_SZ as u32, orig_bytes)));
            assert_eq!(st.installed, "installed,0");
        }

        #[test]
        fn parse_state_rejects_legacy_v1_content() {
            // A bare v1 string or the absent marker is not v2 → None, so restore falls back to v1.
            assert!(parse_state("C:\\old.ico,0").is_none());
            assert!(parse_state(ABSENT_MARKER).is_none());
        }

        #[test]
        fn regtype_round_trips_through_its_discriminant() {
            for t in [REG_SZ, REG_EXPAND_SZ, REG_BINARY, REG_DWORD, REG_QWORD, REG_NONE] {
                assert_eq!(regtype_from_u32(t.clone() as u32), t);
            }
            // An unknown discriminant degrades to REG_BINARY (bytes still restored verbatim).
            assert_eq!(regtype_from_u32(0xDEAD_BEEF), REG_BINARY);
        }
    }
}
