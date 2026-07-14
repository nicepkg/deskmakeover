//! `icons.exportCompare` + the UTC filename-stamp / atomic export-file helpers.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use dm_contracts::{IconOpResultDto, ToastDto};

use super::IconHost;

impl IconHost {
    /// `icons.exportCompare`: save the webview-composed before/after sheet. Composition lives in
    /// the frontend (it owns the fonts, the CJK stack, and both image states — oracle
    /// `ComparisonImageExporter`); this side validates the payload IS a decodable PNG and writes
    /// it to the Pictures folder (fallback: the app's own exports dir). Failure stays honest —
    /// `ok:false` + the failed toast, never a phantom success with no artifact on disk.
    pub fn export_compare(
        &self,
        png_base64: &str,
        pictures: Option<PathBuf>,
    ) -> Result<IconOpResultDto, String> {
        let persisted = self.get_persisted()?;
        let saved = (|| -> Result<PathBuf, String> {
            use base64::Engine;
            // Bounded input (codex icons2-🟠8): the sheet is a ~1200x660 PNG — cap the encoded
            // and decoded sizes so a hostile payload cannot balloon memory, and accept ONLY a
            // real PNG (the image crate would happily decode other formats that then land on
            // disk under a lying `.png` name).
            if png_base64.len() > 12_000_000 {
                return Err("compare sheet: payload too large".into());
            }
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(png_base64.trim())
                .map_err(|e| format!("compare sheet: bad base64: {e}"))?;
            if bytes.len() > 9_000_000 {
                return Err("compare sheet: decoded payload too large".into());
            }
            if !bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
                return Err("compare sheet: not a PNG".into());
            }
            let img = image::load_from_memory(&bytes)
                .map_err(|e| format!("compare sheet: not a decodable image: {e}"))?;
            if (img.width() as u64) * (img.height() as u64) > 16_000_000 {
                return Err("compare sheet: dimensions out of range".into());
            }
            let dir = pictures.unwrap_or_else(|| self.export_fallback_dir.clone());
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            create_export_file(&dir, &utc_stamp(now_secs()), &bytes)
        })();
        Ok(match saved {
            Ok(path) => IconOpResultDto {
                ok: true,
                toast: Some(ToastDto {
                    key: "Toast_CompareSaved".into(),
                    arg: Some(path.display().to_string()),
                }),
                persisted,
            },
            Err(e) => {
                log::warn!("exportCompare failed: {e}");
                IconOpResultDto {
                    ok: false,
                    toast: Some(ToastDto { key: "Toast_CompareFailed".into(), arg: None }),
                    persisted,
                }
            }
        })
    }
}

pub(crate) fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// UTC civil date-time from unix seconds — Howard Hinnant's civil-from-days
/// algorithm, so the host needs no calendar dependency.
fn civil(secs: i64) -> (i64, i64, i64, i64, i64, i64) {
    let days = secs.div_euclid(86_400);
    let tod = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d, tod / 3600, (tod % 3600) / 60, tod % 60)
}

/// `YYYYMMDD-HHMMSS` (UTC) for export filenames. UTC (not local) keeps it
/// deterministic; the stamp is a filename, not a displayed date.
pub(super) fn utc_stamp(secs: i64) -> String {
    let (y, m, d, h, min, s) = civil(secs);
    format!("{y:04}{m:02}{d:02}-{h:02}{min:02}{s:02}")
}

/// ISO-8601 UTC (`YYYY-MM-DDTHH:MM:SSZ`) — preset manifest timestamps (spec 09 §2).
pub(crate) fn iso_stamp(secs: i64) -> String {
    let (y, m, d, h, min, s) = civil(secs);
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{min:02}:{s:02}Z")
}

/// Writes the export ATOMICALLY under a non-clobbering name: `DeskMakeover-<stamp>.png`,
/// suffixed `-2`, `-3`, … on collision. `create_new` makes claim + create one syscall — no
/// check-then-write window (codex icons2-🟠9) — and candidate exhaustion FAILS rather than
/// falling back to an overwrite.
fn create_export_file(dir: &Path, stamp: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    use std::io::Write;
    for n in 1..100u32 {
        let path = if n == 1 {
            dir.join(format!("DeskMakeover-{stamp}.png"))
        } else {
            dir.join(format!("DeskMakeover-{stamp}-{n}.png"))
        };
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut f) => {
                f.write_all(bytes).map_err(|e| e.to_string())?;
                return Ok(path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("compare sheet: export name space exhausted for this second".into())
}
