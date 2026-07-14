//! Windows icon source extraction ([WINDOWS-VERIFY] runtime).
//!
//! Extracts an item's 256px source(s) for the compositor. Ported from the retired C#
//! `ShellIconCanvasSource.cs` (removed from the repo 2026-07-14, ADR-0019):
//! - Shortcut kinds carrying an explicit icon resource honour its `(location, index)` FIRST via
//!   `PrivateExtractIconsW` (the only API that picks the best frame for 256px; `ExtractIconExW` is
//!   the classic 32px fallback), then fall back to the shell image.
//! - Folders / files / everything else ask `IShellItemImageFactory::GetImage`
//!   (`SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK`) for the EXACT image Explorer shows (custom folders,
//!   file-type icons, Electron/Store shortcuts whose resources the raw extractor cannot read).
//! - The Recycle Bin reads the per-user CLSID `DefaultIcon` values (`full`/`empty`) so BOTH states
//!   ride as sources `[0]`=full, `[1]`=empty (the paired-empty asset the apply packages); a missing
//!   value degrades to the live shell image (single source).
//!
//! Pixel contract: `GetImage` HBITMAPs are PREMULTIPLIED BGRA (un-premultiplied here, exactly the
//! oracle's `r*255/a` clamp); `HICON` colour planes are STRAIGHT BGRA (alpha kept, with the classic
//! all-zero-alpha mask fallback for pre-XP resources). Output is a straight-alpha RGBA PNG, the
//! same contract the dev host synthesizes.
//!
//! Ledger-aware re-scan: when the caller proves the live surface is our own styled output, the
//! captured [`RestoreAnchor`](dm_domain::RestoreAnchor) rides in and extraction derives the TRUE
//! original from the anchor material (original `.lnk`/`.url` bytes, original `desktop.ini` icon,
//! original bin registry values) instead of compounding `Style(Style(orig))`.
//!
//! Everything COM/GDI runs on the shared STA apartment; the pure pixel/parse helpers are
//! cross-platform and unit-tested on the Mac host. Runtime behaviour is [WINDOWS-VERIFY].

/// The compositor's master edge: extraction requests this size (the shell may return smaller for
/// low-res-only resources; the DTO carries real dimensions, so that is honest, not an error).
#[cfg(windows)]
const ICON_PX: i32 = 256;

/// Extracts 256px icon sources on the STA apartment (the dev host substitutes its own extractor
/// off-Windows, so the struct itself is Windows-only; the pure helpers above/below are not).
#[cfg(windows)]
pub struct WindowsIconSourceExtractor {
    exec: std::sync::Arc<crate::com::StaExecutor>,
}

#[cfg(windows)]
impl WindowsIconSourceExtractor {
    pub fn new(exec: std::sync::Arc<crate::com::StaExecutor>) -> Self {
        Self { exec }
    }
}

#[cfg(windows)]
impl dm_domain::IconSourceExtractor for WindowsIconSourceExtractor {
    fn extract(
        &self,
        item: &dm_domain::DesktopItem,
        original: Option<&dm_domain::RestoreAnchor>,
    ) -> dm_domain::PortResult<Vec<dm_domain::DecodedImage>> {
        let item = item.clone();
        let original = original.cloned();
        self.exec.run(move || win::extract_blocking(&item, original.as_ref()))?
    }
}

// ---------------------------------------------------------------------------------------------
// Pure helpers — cross-platform, unit-tested on the Mac host.
// ---------------------------------------------------------------------------------------------

/// Converts top-down PREMULTIPLIED BGRA rows (an `IShellItemImageFactory` HBITMAP) to straight
/// RGBA, the oracle's exact math: for 0 < a < 255, `c = min(255, c*255/a)`; a==0 keeps the
/// (necessarily zero) colour, a==255 needs no scaling.
#[cfg(any(windows, test))]
fn premul_bgra_to_rgba(bits: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bits.len());
    for px in bits.chunks_exact(4) {
        let (b, g, r, a) = (px[0], px[1], px[2], px[3]);
        let un = |c: u8| -> u8 {
            if a > 0 && a < 255 {
                ((c as u32 * 255) / a as u32).min(255) as u8
            } else {
                c
            }
        };
        out.extend_from_slice(&[un(r), un(g), un(b), a]);
    }
    out
}

/// Converts top-down STRAIGHT BGRA rows (an HICON colour plane) to RGBA. If EVERY alpha byte is
/// zero the resource predates alpha icons — `mask` (top-down 32bpp render of the AND mask, white =
/// transparent) supplies binary coverage instead.
#[cfg(any(windows, test))]
fn straight_bgra_to_rgba(bits: &[u8], mask: Option<&[u8]>) -> Vec<u8> {
    let legacy = bits.chunks_exact(4).all(|px| px[3] == 0);
    let mut out = Vec::with_capacity(bits.len());
    for (i, px) in bits.chunks_exact(4).enumerate() {
        let a = if legacy {
            match mask {
                // Mask black (0,0,0) = opaque; anything bright = transparent.
                Some(m) => {
                    let o = i * 4;
                    if m.get(o).copied().unwrap_or(0xFF) < 0x80 { 0xFF } else { 0 }
                }
                None => 0xFF, // no mask available → fully opaque beats fully invisible
            }
        } else {
            px[3]
        };
        out.extend_from_slice(&[px[2], px[1], px[0], a]);
    }
    out
}

/// Parses a captured `desktop.ini`'s folder-icon reference into `(location, index)`, covering
/// BOTH forms the shell honours: the modern `IconResource=path,index` and the classic
/// `IconFile=path` + `IconIndex=n` pair (codex icons2-🔴3 sub-point — the old parser saw only
/// `IconResource`). Decoding is UTF-8 → UTF-16LE(BOM) → lossy, so a non-UTF-8 `desktop.ini`
/// still yields its icon path. Section headers are ignored; the `[.ShellClassInfo]` keys are
/// unique enough that a flat key scan is faithful.
#[cfg(any(windows, test))]
fn parse_desktop_ini_icon_ref(bytes: &[u8]) -> Option<(String, i32)> {
    let text = decode_ini_text(bytes);
    let mut icon_file: Option<String> = None;
    let mut icon_index: i32 = 0;
    for line in text.lines() {
        let Some(eq) = line.find('=') else { continue };
        let key = line[..eq].trim();
        let value = line[eq + 1..].trim().trim_matches('"');
        if key.eq_ignore_ascii_case("IconResource") {
            // `path,index` — split the LAST comma (paths can contain commas).
            return Some(match value.rfind(',') {
                Some(c) => (value[..c].trim().to_string(), value[c + 1..].trim().parse().unwrap_or(0)),
                None => (value.to_string(), 0),
            });
        } else if key.eq_ignore_ascii_case("IconFile") {
            icon_file = Some(value.to_string());
        } else if key.eq_ignore_ascii_case("IconIndex") {
            icon_index = value.parse().unwrap_or(0);
        }
    }
    icon_file.map(|f| (f, icon_index))
}

/// Best-effort text decode for a captured `desktop.ini`: UTF-8 (BOM-stripped), else UTF-16LE
/// (BOM), else lossy UTF-8.
#[cfg(any(windows, test))]
fn decode_ini_text(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        if let Ok(s) = std::str::from_utf8(rest) {
            return s.to_string();
        }
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        let u16s: Vec<u16> =
            rest.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        return String::from_utf16_lossy(&u16s);
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => String::from_utf8_lossy(bytes).into_owned(),
    }
}

/// Resolves a possibly-RELATIVE `desktop.ini` icon path against its folder. `%ENV%` and
/// drive-absolute / UNC paths pass through unchanged; a leading `.\`/bare name binds to the
/// folder (the shell resolves a folder's `desktop.ini` icon relative to that folder).
#[cfg(any(windows, test))]
fn resolve_relative(folder: &str, location: &str) -> String {
    let loc = location.trim();
    let is_absolute = loc.starts_with('%')
        || loc.starts_with("\\\\")
        || loc.starts_with("//")
        || loc.get(1..2) == Some(":"); // X:\...
    if is_absolute || folder.is_empty() {
        return loc.to_string();
    }
    let sep = if folder.contains('\\') { '\\' } else { '/' };
    let base = folder.trim_end_matches(['\\', '/']);
    let rel = loc.trim_start_matches("./").trim_start_matches(".\\");
    format!("{base}{sep}{rel}")
}

/// Rasterizes a monochrome HICON (`hbmColor == NULL`) from its DOUBLE-height mask: the top half
/// is the AND plane (white = transparent, black = draw), the bottom half the XOR plane carrying
/// the 1-bit colour. Both arrive as top-down 32bpp BGRA renders of the respective half. An
/// AND-white + XOR-white "invert screen" pixel has no still-image meaning — rendered transparent.
#[cfg(any(windows, test))]
fn mono_planes_to_rgba(and_plane: &[u8], xor_plane: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(xor_plane.len());
    for (a_px, x_px) in and_plane.chunks_exact(4).zip(xor_plane.chunks_exact(4)) {
        if a_px[0] < 0x80 {
            out.extend_from_slice(&[x_px[2], x_px[1], x_px[0], 0xFF]);
        } else {
            out.extend_from_slice(&[0, 0, 0, 0]);
        }
    }
    out
}

/// Expands `%VAR%` environment references the way the shell does for `DefaultIcon` values
/// (`%SystemRoot%\system32\imageres.dll`). An unset variable is left literal.
#[cfg(any(windows, test))]
fn expand_env(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        match rest[start + 1..].find('%') {
            Some(len) => {
                let name = &rest[start + 1..start + 1 + len];
                match std::env::var(name) {
                    Ok(v) => out.push_str(&v),
                    Err(_) => {
                        out.push('%');
                        out.push_str(name);
                        out.push('%');
                    }
                }
                rest = &rest[start + len + 2..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

// ---------------------------------------------------------------------------------------------
// The Windows extraction body (STA thread). [WINDOWS-VERIFY] runtime.
// ---------------------------------------------------------------------------------------------

#[cfg(windows)]
mod win {
    use dm_domain::{
        DecodedImage, DesktopItem, DesktopIniAnchor, ItemKind, PortError, PortResult,
        RecycleBinAnchor, RestoreAnchor, SystemIconAnchor,
    };
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::Foundation::{HWND, SIZE};
    use windows::Win32::Graphics::Gdi::{
        DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HDC,
    };
    use windows::Win32::UI::Shell::{
        ExtractIconExW, SHCreateItemFromParsingName, SHDefExtractIconW, IShellItemImageFactory,
        SIIGBF_BIGGERSIZEOK, SIIGBF_ICONONLY,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        DestroyIcon, GetIconInfo, PrivateExtractIconsW, HICON, ICONINFO,
    };

    use super::{
        expand_env, mono_planes_to_rgba, parse_desktop_ini_icon_ref, premul_bgra_to_rgba,
        resolve_relative, straight_bgra_to_rgba, ICON_PX,
    };
    use crate::apply::recyclebin;
    use crate::classify::parse_icon_location;

    pub(super) fn extract_blocking(
        item: &DesktopItem,
        original: Option<&RestoreAnchor>,
    ) -> PortResult<Vec<DecodedImage>> {
        // Ledger-aware first (codex extractor-review 🔴1): the caller passing an anchor has
        // PROVEN the live surface is our own styled output — re-reading it would compound
        // Style(Style(orig)). The anchor path is TERMINAL (codex icons2-🔴3): the host only
        // passes an anchor once it has PROVEN live == last_applied — the live surface IS our
        // styled output, so a fall-through to the live chain would read Style(orig) and re-bake it
        // into Style(Style(orig)). An anchor that cannot resolve to a TRUSTED original therefore
        // errors (→ per-item degradation upstream), never silently reads live.
        if let Some(anchor) = original {
            return match original_images(item, anchor) {
                Some(images) => Ok(images),
                None => Err(PortError::NotFound(format!(
                    "owned item {}: original anchor unresolvable — degraded (never read the styled live surface)",
                    item.path
                ))),
            };
        }
        if item.kind == ItemKind::RecycleBin {
            return extract_recycle_bin(item);
        }
        // Shortcut kinds honour their explicit icon resource FIRST (the index matters — one DLL
        // holds many icons); everything else asks the shell for Explorer's exact image. Either
        // side falls back to the other, mirroring the oracle's two-way chain.
        let resource = item
            .icon
            .as_ref()
            .and_then(|r| icon_resource_image(&r.location, r.index).ok().flatten());
        let img = if item.kind.is_shortcut() {
            resource.or_else(|| shell_item_image(&item.path).ok().flatten())
        } else {
            shell_item_image(&item.path).ok().flatten().or(resource)
        };
        match img {
            Some(i) => Ok(vec![i]),
            None => Err(PortError::NotFound(format!("no icon image extractable for {}", item.path))),
        }
    }

    // ---- Original-anchor extraction (the user's TRUE source while the live icon is ours) ----

    fn original_images(item: &DesktopItem, anchor: &RestoreAnchor) -> Option<Vec<DecodedImage>> {
        match anchor {
            RestoreAnchor::FileBytes { bytes } => {
                original_from_file_bytes(item, bytes).map(|i| vec![i])
            }
            RestoreAnchor::Folder { desktop_ini, .. } => {
                original_folder_image(&item.path, desktop_ini.as_ref()).map(|i| vec![i])
            }
            // A wrapped loose file: our styling lives on the companion wrapper `.lnk`; the file
            // itself still carries its own type icon — the live shell read IS the original.
            RestoreAnchor::RegularFile(_) => {
                shell_item_image(&item.path).ok().flatten().map(|i| vec![i])
            }
            RestoreAnchor::RecycleBin(a) => original_recycle_bin(a),
            RestoreAnchor::SystemIcon(a) => original_system(a),
            RestoreAnchor::CaptureFailed { .. } => None,
        }
    }

    /// The original System icon. The anchor's `value` is the RAW per-user override (restore-exact);
    /// when it is empty — our own restore leaves an empty per-user key — the original was the MACHINE
    /// default, so read that LIVE (styling never touches HKCR, so the machine default is still the true
    /// original). This recovers the source for a re-style after a reset cycle WITHOUT polluting the
    /// restore-critical `value` (codex re-review 🟠). The live shell image is never re-read here (it
    /// would be OUR styled icon → `Style(Style(original))`); a value that resolves to nothing is
    /// unrecoverable → `None` → terminal degrade upstream.
    fn original_system(a: &SystemIconAnchor) -> Option<Vec<DecodedImage>> {
        let value = match &a.value {
            Some(v) => Some(v.clone()),
            None => crate::apply::system::machine_value(&a.clsid).ok().flatten(),
        };
        value.and_then(|v| resource_from_value(&v.raw)).map(|i| vec![i])
    }

    /// Atomically creates an unpredictable scratch file under `%TEMP%` and writes `bytes`,
    /// returning its path (codex icons2-🟠12: the old predictable PID+counter name + plain
    /// `fs::write` could clobber a pre-planted file or reparse/hardlink). The name mixes PID, a
    /// process-lifetime counter, and a nanosecond clock; `create_new` fails on any pre-existing
    /// entry (a symlink/hardlink an attacker planted included), and the loop reseeds on collision.
    fn write_scratch(stem: &str, ext: &str, bytes: &[u8]) -> Option<std::path::PathBuf> {
        use std::io::Write;
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir();
        for _ in 0..8 {
            let n = N.fetch_add(1, Ordering::Relaxed);
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let path =
                dir.join(format!("dm-orig-{stem}-{}-{n}-{nanos:x}.{ext}", std::process::id()));
            match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut f) => {
                    f.write_all(bytes).ok()?;
                    return Some(path);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => return None,
            }
        }
        None
    }

    /// An unpredictable, freshly-created scratch DIRECTORY under `%TEMP%` (same anti-clobber
    /// discipline as `write_scratch`). `create_dir` fails on a pre-existing entry.
    fn scratch_dir(stem: &str) -> Option<std::path::PathBuf> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir();
        for _ in 0..8 {
            let n = N.fetch_add(1, Ordering::Relaxed);
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let path = dir.join(format!("dm-orig-{stem}-{}-{n}-{nanos:x}", std::process::id()));
            match std::fs::create_dir(&path) {
                Ok(()) => return Some(path),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => return None,
            }
        }
        None
    }

    /// The original `.lnk`/`.url` icon, derived from the CAPTURED file bytes: parse the original
    /// icon location out of them (a `.url` is INI text; a `.lnk` is materialized as a temp sibling
    /// so `IShellLink` reads it), falling back to the shell image of the materialized original —
    /// which resolves the original TARGET's icon exactly as the desktop did before we styled it.
    fn original_from_file_bytes(item: &DesktopItem, bytes: &[u8]) -> Option<DecodedImage> {
        if item.kind == ItemKind::UrlShortcut {
            let text = String::from_utf8_lossy(bytes);
            if let Some((loc, idx)) = crate::textfmt::parse_internet_shortcut_icon(&text) {
                if let Some(img) = icon_resource_image(&loc, idx).ok().flatten() {
                    return Some(img);
                }
            }
        }
        let ext = if item.kind == ItemKind::UrlShortcut { "url" } else { "lnk" };
        let tmp = write_scratch("link", ext, bytes)?;
        let tmp_str = tmp.to_str()?.to_string();
        let img = (|| {
            if ext == "lnk" {
                if let Some((loc, idx)) =
                    crate::shell::shell_link::read_icon_location(&tmp_str).ok().flatten()
                {
                    if let Some(img) = icon_resource_image(&loc, idx).ok().flatten() {
                        return Some(img);
                    }
                }
            }
            shell_item_image(&tmp_str).ok().flatten()
        })();
        let _ = std::fs::remove_file(&tmp);
        img
    }

    /// The original folder icon: the captured `desktop.ini`'s icon reference — `IconResource`
    /// AND the classic `IconFile`/`IconIndex` pair (codex icons2-🔴3 sub-point), with a relative
    /// `location` resolved against the folder itself — else the stock folder icon (a throwaway
    /// empty directory gives the shell's theme-correct rendering; `shell32.dll,3` is the classic
    /// fallback).
    fn original_folder_image(folder: &str, ini: Option<&DesktopIniAnchor>) -> Option<DecodedImage> {
        if let Some(ini) = ini {
            if let Some((loc, idx)) = parse_desktop_ini_icon_ref(&ini.content) {
                let resolved = resolve_relative(folder, &loc);
                if let Some(img) = icon_resource_image(&resolved, idx).ok().flatten() {
                    return Some(img);
                }
            }
        }
        // A throwaway empty directory renders the shell's theme-correct stock folder icon.
        // `create_dir` (not `_all`) fails on any pre-existing entry — same anti-clobber posture
        // as `write_scratch`.
        if let Some(tmp) = scratch_dir("folder") {
            let img = tmp.to_str().and_then(|p| shell_item_image(p).ok().flatten());
            let _ = std::fs::remove_dir(&tmp);
            if img.is_some() {
                return img;
            }
        }
        icon_resource_image(r"%SystemRoot%\system32\shell32.dll", 3).ok().flatten()
    }

    /// The original Recycle Bin pair from the CAPTURED registry values (`read_current()` would
    /// read back our own styled ICOs). `full` is tried first, then `default` — INDEPENDENTLY, so
    /// a present-but-unresolvable `full` still falls through to a valid `default` (codex
    /// icons2-🔴3: the old `full.or(default)` short-circuited on the `Option` ref, never trying
    /// `default` once `full` existed). Anchor fully unusable → `None` → terminal degrade upstream.
    fn original_recycle_bin(a: &RecycleBinAnchor) -> Option<Vec<DecodedImage>> {
        let full = a
            .full
            .as_ref()
            .and_then(|v| resource_from_value(&v.raw))
            .or_else(|| a.default.as_ref().and_then(|v| resource_from_value(&v.raw)))?;
        let empty =
            a.empty.as_ref().and_then(|v| resource_from_value(&v.raw)).unwrap_or_else(|| full.clone());
        Some(vec![full, empty])
    }

    /// Recycle Bin: `[0]` the FULL-state icon, `[1]` the EMPTY-state icon, both resolved from the
    /// effective `DefaultIcon` registry values so the pair is state-independent (the live shell
    /// image would only show the CURRENT state). A missing value degrades to the shell image /
    /// a single source — the apply then simply has no paired-empty asset to package.
    fn extract_recycle_bin(item: &DesktopItem) -> PortResult<Vec<DecodedImage>> {
        let anchor = recyclebin::read_current()?;
        // A present-but-EMPTY per-user key (our own restore leaves one) yields all-None raw values,
        // yet the TRUE original is the MACHINE default — NOT the live shell image, which may be our own
        // styled icon (→ Style(Style(original))). Recover it from HKCR, the same live-machine fallback
        // original_system uses for System icons (codex R2 D-2). Only for the empty-key case; the normal
        // no-override path already carries machine values via read_current.
        let anchor = if anchor.key_existed
            && anchor.full.is_none()
            && anchor.empty.is_none()
            && anchor.default.is_none()
        {
            recyclebin::machine_state().unwrap_or(anchor)
        } else {
            anchor
        };
        let full = anchor
            .full
            .as_ref()
            .or(anchor.default.as_ref())
            .and_then(|v| resource_from_value(&v.raw))
            .or_else(|| shell_item_image(&item.path).ok().flatten());
        let empty = anchor.empty.as_ref().and_then(|v| resource_from_value(&v.raw));
        let full = match full {
            Some(i) => i,
            None => {
                return Err(PortError::NotFound("recycle-bin: no extractable full-state icon".into()))
            }
        };
        // The bin ALWAYS advertises two sources: the driver's registry mutation is a coupled
        // full+empty write, so a single-source scan would let the user bake an apply the driver must
        // then reject wholesale (codex). A missing/unreadable empty value degrades to the full-state
        // image standing in for both — the oracle's behaviour — never a 1-length shape.
        let empty = empty.unwrap_or_else(|| full.clone());
        Ok(vec![full, empty])
    }

    fn resource_from_value(value: &str) -> Option<DecodedImage> {
        let (path, index) = parse_icon_location(value);
        icon_resource_image(&path, index).ok().flatten()
    }

    /// Extracts the best ≤`ICON_PX` frame of `location[,index]` as an HICON and rasterizes it.
    fn icon_resource_image(location: &str, index: i32) -> PortResult<Option<DecodedImage>> {
        let path = expand_env(location);
        if path.trim().is_empty() || !std::path::Path::new(&path).exists() {
            return Ok(None);
        }
        // `PrivateExtractIconsW` takes a fixed NUL-terminated MAX_PATH (260) wide buffer — a
        // windows-rs projection limit, not ours. A longer path skips it and rides
        // `SHDefExtractIconW` below, which takes a plain PCWSTR at any length (codex 🟠7).
        let mut wide = [0u16; 260];
        let mut units: Vec<u16> = path.encode_utf16().collect();
        let fits = units.len() < wide.len();
        if fits {
            wide[..units.len()].copy_from_slice(&units);
        }
        units.push(0); // NUL terminator for the PCWSTR callee
        // SAFETY: plain Win32 icon extraction; every returned HICON is destroyed below.
        unsafe {
            let mut icons = [HICON::default(); 1];
            let mut ids = [0u32; 1];
            let got = if fits {
                PrivateExtractIconsW(
                    &wide,
                    index,
                    ICON_PX,
                    ICON_PX,
                    Some(&mut icons),
                    Some(ids.as_mut_ptr()),
                    0,
                )
            } else {
                0
            };
            // Exactly 1 is success: the documented failure sentinel is 0xFFFFFFFF (file vanished /
            // unreadable), which a naive `>= 1` would treat as success over an unset handle (codex).
            let mut icon = if got == 1 { icons[0] } else { HICON::default() };
            if icon.is_invalid() {
                // Size-aware fallback with NO path-length ceiling (`SHDefExtractIconW` still picks
                // the best frame for the requested edge). S_FALSE = "no icon here" leaves the
                // handle unset — guard both.
                let mut best = HICON::default();
                if SHDefExtractIconW(
                    PCWSTR(units.as_ptr()),
                    index,
                    0,
                    Some(&mut best),
                    None,
                    ICON_PX as u32,
                )
                .is_ok()
                    && !best.is_invalid()
                {
                    icon = best;
                }
            }
            if icon.is_invalid() {
                // Classic 32px fallback for resources neither extractor can read.
                let mut large = HICON::default();
                let _ = ExtractIconExW(&HSTRING::from(path.as_str()), index, Some(&mut large), None, 1);
                icon = large;
            }
            if icon.is_invalid() {
                return Ok(None);
            }
            let img = hicon_to_image(icon);
            let _ = DestroyIcon(icon);
            img
        }
    }

    /// The exact image Explorer shows for `path` (`IShellItemImageFactory`, icon-only).
    fn shell_item_image(path: &str) -> PortResult<Option<DecodedImage>> {
        // SAFETY: COM on the STA thread; the HBITMAP is deleted below.
        unsafe {
            let factory: IShellItemImageFactory =
                match SHCreateItemFromParsingName(&HSTRING::from(path), None) {
                    Ok(f) => f,
                    Err(_) => return Ok(None),
                };
            let hbm = match factory.GetImage(
                SIZE { cx: ICON_PX, cy: ICON_PX },
                SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK,
            ) {
                Ok(h) => h,
                Err(_) => return Ok(None),
            };
            let bits = hbitmap_bgra(hbm);
            let _ = DeleteObject(hbm.into());
            let Some((w, h, bgra)) = bits? else { return Ok(None) };
            // GetImage bitmaps are premultiplied — restore straight alpha (oracle math).
            encode_png(w, h, premul_bgra_to_rgba(&bgra)).map(Some)
        }
    }

    /// Rasterizes an HICON via its `ICONINFO` colour plane (straight alpha), with the classic
    /// AND-mask fallback for pre-alpha resources.
    unsafe fn hicon_to_image(icon: HICON) -> PortResult<Option<DecodedImage>> {
        let mut info = ICONINFO::default();
        if GetIconInfo(icon, &mut info).is_err() {
            return Ok(None);
        }
        let color = info.hbmColor;
        let mask = info.hbmMask;
        let result = (|| -> PortResult<Option<DecodedImage>> {
            match hbitmap_bgra(color)? {
                Some((w, h, bgra)) => {
                    let mask_bits = hbitmap_bgra(mask)?.and_then(|(mw, mh, m)| {
                        // A monochrome mask read at the colour size supplies legacy coverage.
                        if mw == w && mh >= h {
                            Some(m[..(w * h * 4) as usize].to_vec())
                        } else {
                            None
                        }
                    });
                    encode_png(w, h, straight_bgra_to_rgba(&bgra, mask_bits.as_deref())).map(Some)
                }
                // Monochrome icon (`hbmColor == NULL`): the mask is DOUBLE-height — the AND plane
                // stacked on the XOR plane (codex 🟠5). GetDIBits already rendered it as 32bpp
                // rows, so the split is a pure pixel transform.
                None => match hbitmap_bgra(mask)? {
                    Some((w, h2, planes)) if h2 > 0 && h2 % 2 == 0 => {
                        let h = h2 / 2;
                        let half = (w * h * 4) as usize;
                        let rgba = mono_planes_to_rgba(&planes[..half], &planes[half..]);
                        encode_png(w, h, rgba).map(Some)
                    }
                    _ => Ok(None),
                },
            }
        })();
        if !color.is_invalid() {
            let _ = DeleteObject(color.into());
        }
        if !mask.is_invalid() {
            let _ = DeleteObject(mask.into());
        }
        result
    }

    /// Reads an HBITMAP as top-down 32bpp BGRA rows: `(width, height, bytes)`.
    unsafe fn hbitmap_bgra(hbm: HBITMAP) -> PortResult<Option<(u32, u32, Vec<u8>)>> {
        if hbm.is_invalid() {
            return Ok(None);
        }
        let mut bm = BITMAP::default();
        if GetObjectW(
            hbm.into(),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bm as *mut BITMAP as *mut _),
        ) == 0
            || bm.bmWidth <= 0
            || bm.bmHeight <= 0
        {
            return Ok(None);
        }
        let (w, h) = (bm.bmWidth, bm.bmHeight);
        let mut header = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h, // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = vec![0u8; (w as usize) * (h as usize) * 4];
        let hdc: HDC = GetDC(Some(HWND::default()));
        let got = GetDIBits(
            hdc,
            hbm,
            0,
            h as u32,
            Some(bits.as_mut_ptr() as *mut _),
            &mut header,
            DIB_RGB_COLORS,
        );
        ReleaseDC(Some(HWND::default()), hdc);
        if got == 0 {
            return Ok(None);
        }
        Ok(Some((w as u32, h as u32, bits)))
    }

    /// Encodes straight-alpha RGBA rows as the PNG `DecodedImage` the compositor consumes.
    fn encode_png(width: u32, height: u32, rgba: Vec<u8>) -> PortResult<DecodedImage> {
        use image::ImageEncoder;
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png)
            .write_image(&rgba, width, height, image::ExtendedColorType::Rgba8)
            .map_err(|e| PortError::Io(format!("icon png encode: {e}")))?;
        Ok(DecodedImage { width, height, png })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn premultiplied_bgra_restores_straight_rgba_with_the_oracle_math() {
        // 50%-alpha premultiplied mid-grey: c=64,a=128 → straight c = 64*255/128 = 127.
        let bits = [64u8, 64, 64, 128, /* opaque red */ 0, 0, 255, 255, /* fully transparent */ 0, 0, 0, 0];
        let rgba = premul_bgra_to_rgba(&bits);
        assert_eq!(&rgba[0..4], &[127, 127, 127, 128]);
        assert_eq!(&rgba[4..8], &[255, 0, 0, 255], "opaque needs no scaling; BGR swapped to RGB");
        assert_eq!(&rgba[8..12], &[0, 0, 0, 0], "a=0 keeps the zero colour");
        // The clamp: premul overflow (corrupt input) saturates at 255, never wraps.
        let hot = premul_bgra_to_rgba(&[200, 0, 0, 100]);
        assert_eq!(hot[2], 255, "200*255/100 clamps to 255");
    }

    #[test]
    fn straight_bgra_keeps_alpha_and_falls_back_to_the_mask_only_when_all_zero() {
        // Real alpha present → kept verbatim, no mask consulted.
        let rgba = straight_bgra_to_rgba(&[1, 2, 3, 9, 4, 5, 6, 0], None);
        assert_eq!(rgba, vec![3, 2, 1, 9, 6, 5, 4, 0]);
        // All-zero alpha (legacy icon) → the AND mask decides: black=opaque, white=transparent.
        let mask = [0u8, 0, 0, 255, /* white */ 255, 255, 255, 255];
        let legacy = straight_bgra_to_rgba(&[1, 2, 3, 0, 4, 5, 6, 0], Some(&mask));
        assert_eq!(legacy, vec![3, 2, 1, 255, 6, 5, 4, 0]);
        // All-zero alpha and NO mask → opaque beats invisible.
        let bare = straight_bgra_to_rgba(&[1, 2, 3, 0], None);
        assert_eq!(bare[3], 255);
    }

    #[test]
    fn a_monochrome_double_height_mask_splits_into_and_gated_xor_colour() {
        // 2×1 icon: AND plane [black, white] gates coverage; XOR plane [white, black] is the ink.
        let and_plane = [0u8, 0, 0, 255, /* white = transparent */ 255, 255, 255, 255];
        let xor_plane = [255u8, 255, 255, 255, /* black */ 0, 0, 0, 255];
        let rgba = mono_planes_to_rgba(&and_plane, &xor_plane);
        assert_eq!(&rgba[0..4], &[255, 255, 255, 255], "AND black → opaque, XOR white ink");
        assert_eq!(&rgba[4..8], &[0, 0, 0, 0], "AND white → transparent (invert-screen ignored)");
    }

    #[test]
    fn desktop_ini_icon_ref_covers_both_forms_and_encodings() {
        // Modern IconResource.
        let a = b"[.ShellClassInfo]\r\nIconResource=C:\\ic\\folder.dll,4\r\n";
        assert_eq!(parse_desktop_ini_icon_ref(a), Some((r"C:\ic\folder.dll".into(), 4)));
        // Classic IconFile + IconIndex.
        let b = b"[.ShellClassInfo]\r\nIconFile=custom.ico\r\nIconIndex=2\r\n";
        assert_eq!(parse_desktop_ini_icon_ref(b), Some(("custom.ico".into(), 2)));
        // IconFile with no explicit index defaults to 0.
        let c = b"IconFile=only.ico\r\n";
        assert_eq!(parse_desktop_ini_icon_ref(c), Some(("only.ico".into(), 0)));
        // UTF-8 BOM stripped.
        let d = [&[0xEFu8, 0xBB, 0xBF][..], b"IconResource=x.ico,0"].concat();
        assert_eq!(parse_desktop_ini_icon_ref(&d), Some(("x.ico".into(), 0)));
        // No icon reference → None.
        assert_eq!(parse_desktop_ini_icon_ref(b"[.ShellClassInfo]\r\nConfirmFileOp=0\r\n"), None);
    }

    #[test]
    fn relative_folder_icon_paths_bind_to_the_folder() {
        assert_eq!(
            resolve_relative(r"C:\Users\Dev\Desktop\Work", "custom.ico"),
            r"C:\Users\Dev\Desktop\Work\custom.ico"
        );
        assert_eq!(
            resolve_relative(r"C:\Work", r".\icons\a.ico"),
            r"C:\Work\icons\a.ico"
        );
        // Absolute / env / UNC pass through.
        assert_eq!(resolve_relative(r"C:\Work", r"D:\ext.dll"), r"D:\ext.dll");
        assert_eq!(resolve_relative(r"C:\Work", "%SystemRoot%\\a.dll"), "%SystemRoot%\\a.dll");
        assert_eq!(resolve_relative(r"C:\Work", r"\\srv\share\i.ico"), r"\\srv\share\i.ico");
    }

    #[test]
    fn env_references_expand_and_unknown_ones_stay_literal() {
        std::env::set_var("DM_SRC_TEST_ROOT", "/tmp/root");
        assert_eq!(expand_env("%DM_SRC_TEST_ROOT%/x.dll"), "/tmp/root/x.dll");
        assert_eq!(expand_env("%DM_NO_SUCH_VAR%/x"), "%DM_NO_SUCH_VAR%/x");
        assert_eq!(expand_env("plain"), "plain");
        assert_eq!(expand_env("dangling %half"), "dangling %half");
    }
}
