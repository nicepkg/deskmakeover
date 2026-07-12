//! Windows icon source extraction ([WINDOWS-VERIFY] runtime).
//!
//! Extracts an item's 256px source(s) for the compositor, oracle:
//! `legacy/src/DeskMakeover.App/Preview/ShellIconCanvasSource.cs`:
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
//! Everything COM/GDI runs on the shared STA apartment; the pure pixel/parse helpers are
//! cross-platform and unit-tested on the Mac host. Runtime behaviour is [WINDOWS-VERIFY].

/// The compositor's master edge: extraction requests this size (the shell may return smaller for
/// low-res-only resources; the DTO carries real dimensions, so that is honest, not an error).
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
    fn extract(&self, item: &dm_domain::DesktopItem) -> dm_domain::PortResult<Vec<dm_domain::DecodedImage>> {
        let item = item.clone();
        self.exec.run(move || win::extract_blocking(&item))?
    }
}

// ---------------------------------------------------------------------------------------------
// Pure helpers — cross-platform, unit-tested on the Mac host.
// ---------------------------------------------------------------------------------------------

/// Converts top-down PREMULTIPLIED BGRA rows (an `IShellItemImageFactory` HBITMAP) to straight
/// RGBA, the oracle's exact math: for 0 < a < 255, `c = min(255, c*255/a)`; a==0 keeps the
/// (necessarily zero) colour, a==255 needs no scaling.
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

/// Splits a registry `DefaultIcon`-style value `"C:\path\icons.dll,-3"` into `(path, index)`;
/// a value with no comma is the path with index 0. Surrounding quotes are stripped.
fn parse_icon_location(value: &str) -> (String, i32) {
    let v = value.trim().trim_matches('"');
    match v.rsplit_once(',') {
        Some((path, idx)) => match idx.trim().parse::<i32>() {
            Ok(i) => (path.trim().trim_matches('"').to_string(), i),
            Err(_) => (v.to_string(), 0), // a comma inside the path, not an index
        },
        None => (v.to_string(), 0),
    }
}

/// Expands `%VAR%` environment references the way the shell does for `DefaultIcon` values
/// (`%SystemRoot%\system32\imageres.dll`). An unset variable is left literal.
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
    use dm_domain::{DecodedImage, DesktopItem, ItemKind, PortError, PortResult};
    use windows::core::HSTRING;
    use windows::Win32::Foundation::{HWND, SIZE};
    use windows::Win32::Graphics::Gdi::{
        DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HDC,
    };
    use windows::Win32::UI::Shell::{
        ExtractIconExW, SHCreateItemFromParsingName, IShellItemImageFactory, SIIGBF_BIGGERSIZEOK,
        SIIGBF_ICONONLY,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        DestroyIcon, GetIconInfo, PrivateExtractIconsW, HICON, ICONINFO,
    };

    use super::{expand_env, parse_icon_location, premul_bgra_to_rgba, straight_bgra_to_rgba, ICON_PX};
    use crate::apply::recyclebin;

    pub(super) fn extract_blocking(item: &DesktopItem) -> PortResult<Vec<DecodedImage>> {
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

    /// Recycle Bin: `[0]` the FULL-state icon, `[1]` the EMPTY-state icon, both resolved from the
    /// effective `DefaultIcon` registry values so the pair is state-independent (the live shell
    /// image would only show the CURRENT state). A missing value degrades to the shell image /
    /// a single source — the apply then simply has no paired-empty asset to package.
    fn extract_recycle_bin(item: &DesktopItem) -> PortResult<Vec<DecodedImage>> {
        let anchor = recyclebin::read_current()?;
        let full = anchor
            .full
            .as_ref()
            .or(anchor.default.as_ref())
            .and_then(|v| resource_from_value(&v.raw))
            .or_else(|| shell_item_image(&item.path).ok().flatten());
        let empty = anchor.empty.as_ref().and_then(|v| resource_from_value(&v.raw));
        let mut out = Vec::with_capacity(2);
        match full {
            Some(i) => out.push(i),
            None => {
                return Err(PortError::NotFound("recycle-bin: no extractable full-state icon".into()))
            }
        }
        if let Some(e) = empty {
            out.push(e);
        }
        Ok(out)
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
        // `PrivateExtractIconsW` takes a fixed NUL-terminated MAX_PATH (260) wide buffer; a longer
        // path cannot ride it — skip straight to the classic fallback / the caller's shell image.
        let mut wide = [0u16; 260];
        let units: Vec<u16> = path.encode_utf16().collect();
        let fits = units.len() < wide.len();
        if fits {
            wide[..units.len()].copy_from_slice(&units);
        }
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
            let mut icon = if got >= 1 { icons[0] } else { HICON::default() };
            if icon.is_invalid() {
                // Classic 32px fallback for resources PrivateExtractIcons cannot read.
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
            let Some((w, h, bgra)) = hbitmap_bgra(color)? else { return Ok(None) };
            let mask_bits = hbitmap_bgra(mask)?.and_then(|(mw, mh, m)| {
                // A monochrome mask read at the colour size; a double-height XOR+AND legacy mask
                // (colourless icon) is not handled here — colourless icons take the legacy branch
                // with the top half, which is the AND plane.
                if mw == w && mh >= h {
                    Some(m[..(w * h * 4) as usize].to_vec())
                } else {
                    None
                }
            });
            encode_png(w, h, straight_bgra_to_rgba(&bgra, mask_bits.as_deref())).map(Some)
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
    fn icon_location_values_parse_path_and_index() {
        assert_eq!(parse_icon_location(r"C:\w\imageres.dll,-55"), (r"C:\w\imageres.dll".into(), -55));
        assert_eq!(parse_icon_location(r"C:\plain.ico"), (r"C:\plain.ico".into(), 0));
        assert_eq!(parse_icon_location(r#""C:\q uoted.ico",3"#), (r"C:\q uoted.ico".into(), 3));
        // A comma inside the path with no numeric tail is NOT an index.
        assert_eq!(parse_icon_location(r"C:\a,b\i.ico"), (r"C:\a,b\i.ico".into(), 0));
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
