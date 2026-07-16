//! LPE guards for the overlay ICO (ADR-0021 §4). Two independent concerns:
//!
//! * a cheap **resource** pre-check ([`check_size`]) — reject an empty or over-cap file BEFORE
//!   its bytes are read, so an untrusted caller path can never make the elevated helper slurp an
//!   arbitrarily large file into memory. The cap is helper policy, not a container property, so
//!   it lives here rather than in the codec;
//! * full **structural** validation ([`validate_ico`]) — delegated to the single codec truth
//!   source `dm_icon_codec::parse` (M5.11), the same reader the transaction driver bakes with.
//!
//! Together they guarantee the registry only ever points at a validated, DeskMakeover-owned
//! `%ProgramData%` copy — never a caller-supplied path.

use std::path::Path;

use dm_icon_codec::parse;

/// The custom overlay ICO size cap (oracle: `MaxCustomIcoBytes`).
pub const MAX_ICO_BYTES: u64 = 5 * 1024 * 1024;

/// Resource pre-check on the file's declared size, run against `fs::metadata` BEFORE the bytes
/// are read: rejects an empty file and anything past [`MAX_ICO_BYTES`]. Bounding the read is the
/// point — structural validation ([`validate_ico`]) then works on an in-memory buffer that can
/// never exceed the cap.
pub fn check_size(size_bytes: u64) -> Result<(), String> {
    if size_bytes == 0 {
        return Err("overlay ico is empty".to_string());
    }
    if size_bytes > MAX_ICO_BYTES {
        return Err(format!("overlay ico size {size_bytes} exceeds the {MAX_ICO_BYTES}-byte cap"));
    }
    Ok(())
}

/// Full structural validation of the overlay ICO, delegated to `dm_icon_codec::parse` — the
/// single truth source that also gates the baked-asset corpus. It checks the ICONDIR magic
/// (reserved = 0, type = 1, count ≥ 1) AND every deeper invariant: tightly packed monotonic
/// offsets, each `bytesInRes` equal to the exact DIB size, and per-frame `BITMAPINFOHEADER`
/// sanity. That makes it strictly stronger than a 6-byte magic peek — a file with a valid
/// ICONDIR but a truncated or tampered body is now rejected, closing a spoof the old check missed.
pub fn validate_ico(bytes: &[u8]) -> Result<(), String> {
    parse(bytes).map(|_| ()).map_err(|e| format!("overlay file is not a valid .ico: {e}"))
}

/// Rejects a `--file` path SHAPE a privileged helper must never open (audit F6): a UNC path
/// (`\\server\share\…`, which authenticates as SYSTEM to an attacker's server), the device/extended
/// namespaces (`\\.\`, `\\?\`), and any non-drive-absolute path (a bare relative path resolves
/// against the helper's cwd). This is a portable SHAPE check only (unit-tested on the host); it does
/// NOT resolve the filesystem. ⚠️ [WINDOWS-VERIFY, NOT YET CLOSED] drive-absolute syntax still does
/// not prove a LOCAL file: an intermediate junction/symlink (`C:\Users\x\link\icon.ico`) or a mapped
/// drive letter (`Z:` → `\\server\share`) can redirect the open, possibly to UNC. Closing that needs
/// `read_capped_ico` to open with `CreateFileW` + `FILE_FLAG_OPEN_REPARSE_POINT` and verify the final
/// resolved path stays local + under the sanctioned ProgramData staging root — done on the box.
pub fn validate_overlay_path(path: &str) -> Result<(), String> {
    let norm = path.trim().replace('/', "\\");
    if norm.is_empty() {
        return Err("overlay --file path is empty".to_string());
    }
    if norm.starts_with("\\\\") {
        return Err(format!("overlay --file must be a local path, not UNC/device: {path:?}"));
    }
    let b = norm.as_bytes();
    let drive_absolute = b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\';
    if !drive_absolute {
        return Err(format!("overlay --file must be a drive-absolute path: {path:?}"));
    }
    Ok(())
}

/// Opens `path` ONCE and returns its validated overlay-ICO bytes through that single handle.
///
/// This closes a TOCTOU: the previous flow called `metadata()` and then `read()` on the path
/// separately, so a caller could swap a small file for a large one between the size check and the
/// read to smuggle an over-cap payload past [`MAX_ICO_BYTES`]. Here every check runs against one
/// open handle — the file object is fixed at open time, immune to a later path swap. It also
/// rejects non-regular files (a directory reached via junction has `is_file() == false`), caps
/// the read itself so a file that grows after `fstat` is still bounded, and validates the
/// container structurally via [`validate_ico`].
pub fn read_capped_ico(path: &Path) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut file = open_no_follow(path)?;
    // A2 (closes the [WINDOWS-VERIFY, NOT YET CLOSED] gap on validate_overlay_path): a syntactic
    // drive-absolute check cannot prove LOCALITY — an intermediate junction or a mapped drive letter
    // (`Z:` → `\\server\share`) can redirect the open to a UNC/remote target and make the elevated
    // helper read (or authenticate to) a place it must never touch. Verify the OPENED handle's final
    // resolved path is a local FIXED disk before reading a byte. [WINDOWS-VERIFY] the junction /
    // mapped-drive cases on the box; the local classification is host-tested.
    #[cfg(windows)]
    assert_local_fixed_path(&file)?;
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Err("overlay --file is not a regular file".to_string());
    }
    check_size(meta.len())?; // cheap pre-check on the OPEN handle's length, not a re-resolved path
    let mut bytes = Vec::new();
    // Bound the read regardless of what the handle reports: reading MAX+1 lets check_size reject a
    // file that grew past the cap after fstat (and an empty file, which yields zero bytes).
    file.by_ref().take(MAX_ICO_BYTES + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    check_size(bytes.len() as u64)?;
    validate_ico(&bytes)?;
    Ok(bytes)
}

/// Open for read WITHOUT following a final-component reparse point (junction/symlink), so a link
/// planted AS the target opens the link object (which then fails the `is_file` check) rather than its
/// target. Intermediate junctions are still resolved by the OS, but [`assert_local_fixed_path`]
/// catches any resulting remote redirect. Non-Windows keeps the plain open (host test builds only).
#[cfg(windows)]
fn open_no_follow(path: &Path) -> Result<std::fs::File, String> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0)
        .open(path)
        .map_err(|e| e.to_string())
}
#[cfg(not(windows))]
fn open_no_follow(path: &Path) -> Result<std::fs::File, String> {
    std::fs::File::open(path).map_err(|e| e.to_string())
}

/// Reject an open handle whose final resolved path is not a LOCAL FIXED disk (A2), returning that
/// resolved `\\?\X:\...` path on success. Two checks a syntactic path test cannot make:
/// `GetFinalPathNameByHandleW` reveals a junction redirect as the `\\?\UNC\...` form
/// ([`is_local_dos_path`] rejects it), and `GetDriveTypeW` rejects a mapped drive letter that
/// resolved to a network share (`DRIVE_REMOTE`) even when it kept its `X:` form. The resolved path
/// is canonical (no `..`, junctions collapsed), so a caller can safely confine it to a root.
#[cfg(windows)]
fn resolve_local_fixed_path(file: &std::fs::File, what: &str) -> Result<String, String> {
    use std::os::windows::io::AsRawHandle;

    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::{
        GetDriveTypeW, GetFinalPathNameByHandleW, FILE_NAME_NORMALIZED, GETFINALPATHNAMEBYHANDLE_FLAGS,
        VOLUME_NAME_DOS,
    };
    // Win32 DRIVE_FIXED; windows-rs 0.61 does not surface it as a const, and GetDriveTypeW returns u32.
    const DRIVE_FIXED: u32 = 3;

    let handle = HANDLE(file.as_raw_handle() as _);
    let mut buf = vec![0u16; 32768]; // the NT max path length — long paths must not truncate
    let flags = GETFINALPATHNAMEBYHANDLE_FLAGS(FILE_NAME_NORMALIZED.0 | VOLUME_NAME_DOS.0);
    // SAFETY: `handle` is a live read handle owned by `file`; `buf` outlives the call.
    let len = unsafe { GetFinalPathNameByHandleW(handle, &mut buf, flags) };
    if len == 0 || len as usize >= buf.len() {
        return Err(format!("could not resolve the {what} final path"));
    }
    let resolved = String::from_utf16_lossy(&buf[..len as usize]);
    if !is_local_dos_path(&resolved) {
        return Err(format!("{what} resolves to a non-local path: {resolved:?}"));
    }
    // Probe the resolved volume (`\\?\X:\` → `X:\`): a mapped network drive is DRIVE_REMOTE.
    let root: Vec<u16> = format!("{}:\\", &resolved[4..5]).encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: `root` is a NUL-terminated wide string that outlives the call.
    let dtype = unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) };
    if dtype != DRIVE_FIXED {
        return Err(format!("{what} is not on a local fixed disk (drive type {dtype}): {resolved:?}"));
    }
    Ok(resolved)
}

#[cfg(windows)]
fn assert_local_fixed_path(file: &std::fs::File) -> Result<(), String> {
    resolve_local_fixed_path(file, "overlay --file").map(|_| ())
}

/// The no-follow open, exposed for the desktop-items batch's other privileged reads (`.lnk` bytes,
/// staged originals, the manifest itself) — a final-component junction opens the link object, not
/// its target.
#[cfg(windows)]
pub fn open_file_no_follow(path: &Path) -> Result<std::fs::File, String> {
    open_no_follow(path)
}

/// Assert an open handle resolves to a local FIXED disk (the resolved path is discarded). `what`
/// names the file in the error message. For non-ICO privileged reads where only the yes/no matters.
#[cfg(windows)]
pub fn assert_handle_local_fixed(file: &std::fs::File, what: &str) -> Result<(), String> {
    resolve_local_fixed_path(file, what).map(|_| ())
}

/// Confirm a desktop-item TARGET the elevated helper is about to write is a real local file whose
/// canonical path sits UNDER one of `roots` (the Public/All-Users desktop or the ProgramData staging
/// root). This is the LPE gate for `apply|restore-desktop-items`: the manifest is untrusted, so the
/// helper must independently prove the destination is a place the unelevated app legitimately could
/// not write — never an arbitrary caller-named path like `System32\...`. Opens without following a
/// final-component reparse point, resolves the true path (junctions/`..`/mapped drives collapsed),
/// asserts local-fixed, then confines it to a root. `roots` are the callers' already-resolved
/// `\\?\X:\...` known-folder paths. [WINDOWS-VERIFY] the live junction/mapped-drive cases on the box.
#[cfg(windows)]
pub fn confirm_target_under_root(path: &std::path::Path, roots: &[String]) -> Result<(), String> {
    let file = open_no_follow(path)?;
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Err(format!("desktop-item target is not a regular file: {}", path.display()));
    }
    let resolved = resolve_local_fixed_path(&file, "desktop-item target")?;
    if !is_under_resolved_root(&resolved, roots) {
        return Err(format!("desktop-item target is outside every privileged root: {resolved:?}"));
    }
    Ok(())
}

/// Pure confinement policy: is `resolved` (a canonical `\\?\X:\...` path from
/// `GetFinalPathNameByHandleW`) strictly INSIDE one of `roots` (each also a resolved `\\?\X:\...`
/// path)? Component-wise + Windows case-insensitive, so `...\PublicDesktop2\x` can never pass as a
/// child of `...\PublicDesktop`. Both sides are already OS-canonical (no `..`, no junctions), so a
/// straight component-prefix test is sound. Host-testable — the security decision is unit-covered.
pub fn is_under_resolved_root(resolved: &str, roots: &[String]) -> bool {
    let target = split_components(resolved);
    if target.is_empty() {
        return false;
    }
    roots.iter().any(|root| {
        let r = split_components(root);
        // STRICTLY inside (a child), never the root itself: target must be longer AND share the
        // whole root prefix component-for-component.
        !r.is_empty() && target.len() > r.len() && r.iter().zip(&target).all(|(a, b)| a == b)
    })
}

/// Split a `\\?\...` path into lowercased path components, dropping the `\\?\` marker and any empty
/// segments. No `..` folding is needed — inputs are OS-resolved canonical paths.
fn split_components(path: &str) -> Vec<String> {
    path.strip_prefix(r"\\?\")
        .unwrap_or(path)
        .split(['\\', '/'])
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_lowercase())
        .collect()
}

/// Pure classifier for a `GetFinalPathNameByHandleW`(VOLUME_NAME_DOS) result: true ONLY for a local
/// DOS drive path `\\?\X:\...`. The `\\?\UNC\...` form is a remote (junction/UNC) redirect. Host-
/// testable so the security-relevant decision is unit-covered on any platform (A2).
#[cfg_attr(not(windows), allow(dead_code))]
fn is_local_dos_path(resolved: &str) -> bool {
    let b = resolved.as_bytes();
    if resolved.len() < 7 || &resolved[..4] != r"\\?\" {
        return false; // GetFinalPathNameByHandleW always returns the \\?\ prefix
    }
    if resolved.len() >= 8 && resolved[4..8].eq_ignore_ascii_case("UNC\\") {
        return false; // \\?\UNC\server\share\... — remote
    }
    b[4].is_ascii_alphabetic() && b[5] == b':' && b[6] == b'\\'
}

#[cfg(test)]
mod tests {
    use super::*;
    use dm_icon_codec::{write_ico, Raster};

    /// A real, structurally valid multi-size ICO built through the codec (no hand-maintained
    /// golden). Corruption tests mutate a clone of this to prove `validate_ico` rejects forgeries.
    fn valid_ico() -> Vec<u8> {
        let frame = |size: usize, r: u8, g: u8, b: u8| {
            let mut raster = Raster::new(size, size);
            for px in raster.data.chunks_exact_mut(4) {
                px[0] = r;
                px[1] = g;
                px[2] = b;
                px[3] = 255;
            }
            raster
        };
        write_ico(&[frame(16, 10, 20, 30), frame(32, 40, 50, 60)])
    }

    #[test]
    fn check_size_accepts_a_normal_file() {
        assert!(check_size(1024).is_ok());
        assert!(check_size(MAX_ICO_BYTES).is_ok());
    }

    #[test]
    fn check_size_rejects_empty_and_oversized() {
        assert!(check_size(0).is_err());
        assert!(check_size(MAX_ICO_BYTES + 1).is_err());
    }

    #[test]
    fn validate_overlay_path_rejects_unc_device_and_relative() {
        assert!(validate_overlay_path(r"\\attacker\share\x.ico").is_err(), "UNC");
        assert!(validate_overlay_path(r"\\.\PhysicalDrive0").is_err(), "device");
        assert!(validate_overlay_path(r"\\?\C:\x.ico").is_err(), "extended/device");
        assert!(validate_overlay_path(r"..\..\evil.ico").is_err(), "relative");
        assert!(validate_overlay_path("overlay.ico").is_err(), "bare relative");
        assert!(validate_overlay_path("").is_err(), "empty");
        assert!(validate_overlay_path(r"C:\ProgramData\DeskMakeover\overlay.ico").is_ok());
        assert!(validate_overlay_path("C:/ProgramData/DeskMakeover/overlay.ico").is_ok(), "forward slashes normalise");
    }

    #[test]
    fn is_local_dos_path_rejects_remote_redirects_and_accepts_local_drives() {
        // A2: the security-relevant classification of a resolved final path. A junction that lands on
        // a share resolves to the \\?\UNC\ form; a genuine local file to \\?\X:\ .
        assert!(is_local_dos_path(r"\\?\C:\ProgramData\DeskMakeover\overlay.ico"));
        assert!(is_local_dos_path(r"\\?\Z:\overlay.ico")); // a drive letter that resolved to a local path
        assert!(!is_local_dos_path(r"\\?\UNC\attacker\share\overlay.ico"), "junction/UNC redirect");
        assert!(!is_local_dos_path(r"\\?\unc\attacker\share\overlay.ico"), "case-insensitive UNC");
        assert!(!is_local_dos_path(r"C:\ProgramData\overlay.ico"), "resolved paths always carry the prefix");
        assert!(!is_local_dos_path(""));
        assert!(!is_local_dos_path(r"\\?\"));
    }

    #[test]
    fn is_under_resolved_root_confines_the_elevated_helper_to_privileged_roots() {
        let public = r"\\?\C:\Users\Public\Desktop".to_string();
        let programdata = r"\\?\C:\ProgramData\DeskMakeover".to_string();
        let roots = vec![public, programdata];
        // A real Public-desktop shortcut and a ProgramData staged icon are inside.
        assert!(is_under_resolved_root(r"\\?\C:\Users\Public\Desktop\Chrome.lnk", &roots));
        assert!(is_under_resolved_root(r"\\?\C:\ProgramData\DeskMakeover\assets\a.ico", &roots));
        // Case-insensitive (Windows).
        assert!(is_under_resolved_root(r"\\?\c:\users\public\desktop\chrome.lnk", &roots));
        // The LPE targets an elevated write must never reach.
        assert!(!is_under_resolved_root(r"\\?\C:\Windows\System32\evil.dll", &roots));
        assert!(!is_under_resolved_root(r"\\?\C:\Users\Public\Documents\x.lnk", &roots), "sibling dir");
        // A prefix that is not a component boundary must not pass (Desktop2 is not under Desktop).
        assert!(!is_under_resolved_root(r"\\?\C:\Users\Public\Desktop2\x.lnk", &roots));
        // The root itself is not a child (we only ever write ITEMS inside it).
        assert!(!is_under_resolved_root(r"\\?\C:\Users\Public\Desktop", &roots));
        // A UNC redirect never carries the local `\\?\X:\` shape resolve_local_fixed_path requires.
        assert!(!is_under_resolved_root(r"\\?\UNC\server\share\Chrome.lnk", &roots));
        assert!(!is_under_resolved_root("", &roots));
        assert!(!is_under_resolved_root(r"\\?\C:\x", &[]));
    }

    #[test]
    fn validate_ico_accepts_a_real_multi_size_icon() {
        assert!(validate_ico(&valid_ico()).is_ok());
    }

    #[test]
    fn validate_ico_rejects_a_png_masquerading_as_an_icon() {
        // PNG magic — reserved u16 is non-zero, so it is not an ICONDIR.
        assert!(validate_ico(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]).is_err());
    }

    #[test]
    fn validate_ico_rejects_a_cursor_type_spoof() {
        // type = 2 is a .cur, not an icon (oracle `IsIco` rejects it; parse agrees).
        let mut bytes = valid_ico();
        bytes[2] = 2;
        assert!(validate_ico(&bytes).is_err());
    }

    #[test]
    fn validate_ico_rejects_a_nonzero_reserved_field() {
        // A spoofed ICONDIR with a non-zero reserved word is not a real icon container.
        let mut bytes = valid_ico();
        bytes[0] = 1;
        assert!(validate_ico(&bytes).is_err());
        let mut bytes = valid_ico();
        bytes[1] = 1;
        assert!(validate_ico(&bytes).is_err());
    }

    #[test]
    fn validate_ico_rejects_a_high_byte_type_spoof() {
        // type stored as a little-endian u16; a value in the high byte (0x0100 = 256) is not 1.
        let mut bytes = valid_ico();
        bytes[3] = 1;
        assert!(validate_ico(&bytes).is_err());
    }

    #[test]
    fn validate_ico_rejects_a_zero_image_count() {
        let mut bytes = valid_ico();
        bytes[4] = 0;
        bytes[5] = 0;
        assert!(validate_ico(&bytes).is_err());
    }

    #[test]
    fn validate_ico_rejects_a_truncated_icondir() {
        assert!(validate_ico(&[0, 0, 1, 0]).is_err()); // fewer than 6 bytes
        assert!(validate_ico(&[]).is_err());
    }

    // ── Structural forgeries the old 6-byte magic peek accepted but `parse` catches ──
    // These are the concrete security win of converging on the codec: a payload with a
    // perfectly valid ICONDIR header yet a dishonest body no longer reaches the registry.

    #[test]
    fn validate_ico_rejects_a_valid_header_with_a_truncated_body() {
        // Keep the ICONDIR + directory entries, drop the DIB payload the entry promises.
        let mut bytes = valid_ico();
        bytes.truncate(6 + 16 * 2); // dir_end for a 2-image ICO; no frame data follows
        assert!(validate_ico(&bytes).is_err());
    }

    #[test]
    fn validate_ico_rejects_a_tampered_image_offset() {
        // Point the first frame's offset somewhere other than the packed position.
        let mut bytes = valid_ico();
        let off = 6 + 12; // ICONDIRENTRY[0].imageOffset (u32 at entry byte 12)
        bytes[off] = bytes[off].wrapping_add(4);
        assert!(validate_ico(&bytes).is_err());
    }

    #[test]
    fn validate_ico_rejects_a_lied_bytes_in_res() {
        // Inflate the first frame's declared payload size past its true DIB size.
        let mut bytes = valid_ico();
        let field = 6 + 8; // ICONDIRENTRY[0].bytesInRes (u32 at entry byte 8)
        bytes[field] = bytes[field].wrapping_add(1);
        assert!(validate_ico(&bytes).is_err());
    }

    // ── read_capped_ico: single-handle read (P2-4 TOCTOU) ──

    #[test]
    fn read_capped_ico_returns_the_bytes_of_a_valid_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("overlay.ico");
        let expected = valid_ico();
        std::fs::write(&path, &expected).unwrap();
        assert_eq!(read_capped_ico(&path).unwrap(), expected);
    }

    #[test]
    fn read_capped_ico_rejects_a_directory() {
        // A directory (e.g. reached through a junction) must never pass as an overlay ICO.
        // Unix: File::open succeeds, then is_file() == false → our "regular file" reject.
        // Windows: File::open on a directory fails outright (no FILE_FLAG_BACKUP_SEMANTICS),
        // so the OS error IS the rejection. Either way the directory is refused.
        let dir = tempfile::tempdir().unwrap();
        let err = read_capped_ico(dir.path()).unwrap_err();
        #[cfg(unix)]
        assert!(err.contains("regular file"), "unexpected error: {err}");
        #[cfg(windows)]
        assert!(!err.is_empty(), "a directory must be rejected, got empty error");
    }

    #[test]
    fn read_capped_ico_rejects_an_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.ico");
        std::fs::write(&path, b"").unwrap();
        assert!(read_capped_ico(&path).is_err());
    }

    #[test]
    fn read_capped_ico_rejects_a_small_non_ico_file() {
        // Within the size cap but not an icon container — structural validation still rejects it.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("not.ico");
        std::fs::write(&path, b"hello, not an icon").unwrap();
        assert!(read_capped_ico(&path).is_err());
    }
}
