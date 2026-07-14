//! The user preset library + `.dmpreset` package reader/writer (spec 09).
//!
//! Division of trust: this module owns STRUCTURE and SECURITY — bounded unzip,
//! zip-slip refusal, string caps, PNG sniffing, atomic non-clobbering writes.
//! Payload SEMANTICS (enum whitelists, clamping) belong to the ONE TS validator
//! (`lib/icon-look.normalizeIconLook`), so `payload_json` is opaque here and the
//! import flow is: `read_package` (pure, nothing written) → TS validates →
//! preview → user confirms → `save` per entry. `save` is the ONLY library writer.
//!
//! Library layout (== unpacked package, spec 09 §1):
//! `data_dir/presets/<entryId>/{entry.json, thumb.png?}`.

use std::io::Read;
use std::path::{Path, PathBuf};

use base64::Engine;
use dm_contracts::{
    PresetEntryDto, PresetMetaDto, PresetPackageReadDto, PresetReadEntryDto, PresetSaveDto,
};
use serde::{Deserialize, Serialize};

/// Container format this build reads/writes (spec 09 §2).
const FORMAT: &str = "dmpreset/1";

// Bounds (spec 09 §4.2) — a hostile pack must die cheap.
const MAX_PACK_BYTES: u64 = 20 * 1024 * 1024;
const MAX_ENTRIES: usize = 64;
const MAX_TOTAL_DECOMPRESSED: u64 = 100 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 4 * 1024 * 1024;
const MAX_THUMB_BYTES: u64 = 2 * 1024 * 1024;
const MAX_PAYLOAD_CHARS: usize = 256 * 1024;
const MAX_COMPRESSION_RATIO: u64 = 200;

// String caps (spec 09 §4.4). Chars, not bytes — CJK names count fairly.
const MAX_NAME: usize = 80;
const MAX_AUTHOR: usize = 80;
const MAX_DESCRIPTION: usize = 500;

const PNG_MAGIC: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
const ZIP_MAGIC: [u8; 4] = [b'P', b'K', 0x03, 0x04];

/// The manifest as serialized into a package (spec 09 §2). The library's
/// per-entry `entry.json` reuses `ManifestEntry` verbatim — one schema.
#[derive(Serialize, Deserialize)]
struct Manifest {
    format: String,
    generator: String,
    #[serde(rename = "createdAt")]
    created_at: String,
    entries: Vec<ManifestEntry>,
}

#[derive(Serialize, Deserialize)]
struct ManifestEntry {
    id: String,
    #[serde(rename = "type")]
    preset_type: String,
    #[serde(rename = "schemaVersion")]
    schema_version: u32,
    meta: MetaJson,
    /// Inlined payload (spec 09 §2): reader-side this is any JSON value; it is
    /// re-serialized to the opaque `payload_json` string the TS validator owns.
    payload: serde_json::Value,
    #[serde(default)]
    thumbnail: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct MetaJson {
    name: String,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(rename = "createdAt", default)]
    created_at: Option<String>,
}

pub struct PresetStore {
    root: PathBuf,
    /// Serializes library-mutating ops (save/delete/rename) so two concurrent
    /// Tauri invocations for the same id can never race on the deterministic
    /// `.tmp-`/`.bak-` staging paths (codex FIX-4 #3).
    write_lock: std::sync::Mutex<()>,
}

impl PresetStore {
    pub fn new(data_dir: &Path) -> Self {
        let store = Self { root: data_dir.join("presets"), write_lock: std::sync::Mutex::new(()) };
        // Recover any interrupted overwrite BEFORE the first command is
        // reachable (codex FIX-4 #1): a crash between "park old at .bak" and
        // "swap new in" must restore the old entry, never leave the library
        // short an entry.
        store.recover();
        store
    }

    /// Heal interrupted swaps (codex FIX-4 #1). Idempotent: a `.bak-<id>` whose
    /// canonical `<id>` is gone = a crash mid-swap → restore it; a `.bak-<id>`
    /// whose canonical exists = a superseded backup → drop it; a `.tmp-<id>` =
    /// incomplete staging → drop it. Leftover swap dirs never accumulate.
    fn recover(&self) {
        let Ok(dirs) = std::fs::read_dir(&self.root) else { return };
        for d in dirs.flatten() {
            let name = d.file_name();
            let Some(name) = name.to_str() else { continue };
            if let Some(id) = name.strip_prefix(".bak-") {
                let canonical = self.root.join(id);
                if canonical.exists() {
                    let _ = std::fs::remove_dir_all(d.path());
                } else if valid_id(id) {
                    let _ = std::fs::rename(d.path(), &canonical);
                } else {
                    let _ = std::fs::remove_dir_all(d.path());
                }
            } else if name.starts_with(".tmp-") {
                let _ = std::fs::remove_dir_all(d.path());
            }
        }
    }

    // ---- read_package: pure, bounded, never writes ------------------------

    /// Read a `.dmpreset` file into per-entry candidates. Fail-closed on the
    /// container level (not a zip / wrong format major / bounds tripped);
    /// per-entry failures are reported individually (partial success, §5).
    pub fn read_package(&self, path: &str) -> PresetPackageReadDto {
        match self.read_package_inner(Path::new(path)) {
            Ok(dto) => dto,
            Err(e) => PresetPackageReadDto { format_ok: false, entries: Vec::new(), error: Some(e) },
        }
    }

    fn read_package_inner(&self, path: &Path) -> Result<PresetPackageReadDto, String> {
        let meta = std::fs::metadata(path).map_err(|e| format!("cannot read file: {e}"))?;
        if !meta.is_file() {
            return Err("not a file".into());
        }
        if meta.len() > MAX_PACK_BYTES {
            return Err(format!("package too large ({} bytes)", meta.len()));
        }
        let file = std::fs::File::open(path).map_err(|e| format!("cannot open: {e}"))?;
        let mut zip = zip::ZipArchive::new(std::io::BufReader::new(file))
            .map_err(|e| format!("not a zip archive: {e}"))?;
        if zip.len() > MAX_ENTRIES + 1 {
            return Err(format!("too many archive entries ({})", zip.len()));
        }
        // Screen EVERY central-directory entry before touching the manifest
        // (codex FIX-6 #5): a hostile entry anywhere — traversal name, ratio
        // bomb, absurd declared size, exotic compression — rejects the whole
        // container, it is not merely left unread (spec 09 §4.1/§4.2).
        screen_archive(&mut zip)?;
        // Global decompression budget across EVERY read from this archive.
        let mut budget = Budget::new(MAX_TOTAL_DECOMPRESSED);

        let manifest_bytes = read_entry_bounded(&mut zip, "manifest.json", MAX_MANIFEST_BYTES, &mut budget)?
            .ok_or_else(|| "manifest.json missing".to_string())?;
        let manifest: Manifest = serde_json::from_slice(&manifest_bytes)
            .map_err(|e| format!("manifest.json malformed: {e}"))?;
        // Format gate: same major reads; anything else is fail-closed (§3).
        if manifest.format != FORMAT {
            return Ok(PresetPackageReadDto {
                format_ok: false,
                entries: Vec::new(),
                error: Some(format!("unsupported format '{}' — update DeskMakeover", manifest.format)),
            });
        }
        if manifest.entries.len() > MAX_ENTRIES {
            return Err(format!("too many entries ({})", manifest.entries.len()));
        }

        let mut out = Vec::new();
        for entry in manifest.entries {
            out.push(self.read_one_entry(&mut zip, entry, &mut budget));
        }
        Ok(PresetPackageReadDto { format_ok: true, entries: out, error: None })
    }

    fn read_one_entry<R: Read + std::io::Seek>(
        &self,
        zip: &mut zip::ZipArchive<R>,
        entry: ManifestEntry,
        budget: &mut Budget,
    ) -> PresetReadEntryDto {
        let fail = |e: String| PresetReadEntryDto { entry: None, thumb_png_base64: None, error: Some(e) };
        if !valid_id(&entry.id) {
            return fail(format!("entry '{}': invalid id", sanitize_str(&entry.preset_type, 16)));
        }
        let meta = match sanitize_meta(&entry.meta) {
            Ok(m) => m,
            Err(e) => return fail(format!("entry {}: {e}", entry.id)),
        };
        if entry.preset_type != "icon" {
            // Reserved types (wallpaper, …) are honest per-entry failures, not junk.
            return fail(format!("{}: type '{}' needs a newer DeskMakeover", meta.name, sanitize_str(&entry.preset_type, 24)));
        }
        let payload_json = entry.payload.to_string();
        if payload_json.chars().count() > MAX_PAYLOAD_CHARS {
            return fail(format!("{}: payload too large", meta.name));
        }
        // Thumbnail: optional; must be a safe archive path + a real PNG, and it
        // is RE-ENCODED by our own codec (codex FIX-6 #4) — the package's raw
        // bytes never reach the webview. A bad thumb DROPS the thumb, it never
        // sinks the entry (it is decoration).
        let thumb = entry
            .thumbnail
            .as_deref()
            .filter(|name| safe_asset_name(name))
            .and_then(|name| read_entry_bounded(zip, name, MAX_THUMB_BYTES, budget).ok().flatten())
            .and_then(|bytes| reencode_thumb_png(&bytes).ok())
            .map(|bytes| base64::engine::general_purpose::STANDARD.encode(bytes));
        PresetReadEntryDto {
            entry: Some(PresetEntryDto {
                id: entry.id,
                preset_type: entry.preset_type,
                schema_version: entry.schema_version,
                meta,
                payload_json,
                has_thumb: thumb.is_some(),
            }),
            thumb_png_base64: thumb,
            error: None,
        }
    }

    // ---- library CRUD ------------------------------------------------------

    /// List every library entry (unreadable/corrupt dirs are skipped, logged).
    pub fn list(&self) -> Vec<PresetEntryDto> {
        let mut out = Vec::new();
        let Ok(dirs) = std::fs::read_dir(&self.root) else { return out };
        for dir in dirs.flatten() {
            let path = dir.path();
            if !path.is_dir() {
                continue;
            }
            // .tmp-/.bak- siblings are swap machinery, never entries.
            if path.file_name().and_then(|n| n.to_str()).is_none_or(|n| n.starts_with('.')) {
                continue;
            }
            match self.load_entry(&path) {
                Ok(entry) => out.push(entry),
                Err(e) => log::warn!("preset library: skipping {}: {e}", path.display()),
            }
        }
        // Stable order: newest first by meta.createdAt, then name.
        out.sort_by(|a, b| {
            b.meta.created_at.cmp(&a.meta.created_at).then_with(|| a.meta.name.cmp(&b.meta.name))
        });
        out
    }

    fn load_entry(&self, dir: &Path) -> Result<PresetEntryDto, String> {
        let raw = std::fs::read(dir.join("entry.json")).map_err(|e| e.to_string())?;
        if raw.len() as u64 > MAX_MANIFEST_BYTES {
            return Err("entry.json too large".into());
        }
        let entry: ManifestEntry = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
        if !valid_id(&entry.id) || Some(entry.id.as_str()) != dir.file_name().and_then(|n| n.to_str()) {
            return Err("entry id does not match its directory".into());
        }
        let meta = sanitize_meta(&entry.meta)?;
        Ok(PresetEntryDto {
            has_thumb: dir.join("thumb.png").is_file(),
            payload_json: entry.payload.to_string(),
            id: entry.id,
            preset_type: entry.preset_type,
            schema_version: entry.schema_version,
            meta,
        })
    }

    /// Save one validated entry into the library — the ONLY library writer.
    /// Never clobbers: an existing id fails unless `overwrite` (the TS side
    /// implements import-as-copy by minting a fresh id, spec 09 §5).
    pub fn save(&self, entry: PresetSaveDto, overwrite: bool) -> Result<PresetEntryDto, String> {
        let _guard = self.write_lock.lock().map_err(|_| "preset store lock poisoned".to_string())?;
        if !valid_id(&entry.id) {
            return Err("invalid entry id".into());
        }
        if entry.preset_type != "icon" {
            return Err(format!("unsupported preset type '{}'", sanitize_str(&entry.preset_type, 24)));
        }
        if entry.payload_json.chars().count() > MAX_PAYLOAD_CHARS {
            return Err("payload too large".into());
        }
        let payload: serde_json::Value =
            serde_json::from_str(&entry.payload_json).map_err(|e| format!("payload not JSON: {e}"))?;
        let meta = sanitize_meta(&MetaJson {
            name: entry.meta.name.clone(),
            author: entry.meta.author.clone(),
            description: entry.meta.description.clone(),
            created_at: entry.meta.created_at.clone(),
        })?;
        let thumb = decode_thumb(entry.thumb_png_base64.as_deref())?;

        let dir = self.entry_dir(&entry.id)?;
        if dir.exists() && !overwrite {
            return Err("exists".into());
        }
        // Stage-first swap (codex FIX-6 #6): the replacement is FULLY written
        // into a sibling before the old entry moves; the old entry is parked at
        // a .bak and removed only after the swap lands — a crash/disk-full at
        // any step leaves either the old or the new entry, never neither.
        let staging = self.root.join(format!(".tmp-{}", entry.id));
        let _ = std::fs::remove_dir_all(&staging);
        std::fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
        let manifest_entry = ManifestEntry {
            id: entry.id.clone(),
            preset_type: entry.preset_type.clone(),
            schema_version: entry.schema_version,
            meta: MetaJson {
                name: meta.name.clone(),
                author: meta.author.clone(),
                description: meta.description.clone(),
                created_at: meta.created_at.clone(),
            },
            payload,
            thumbnail: thumb.as_ref().map(|_| format!("assets/{}/thumb.png", entry.id)),
        };
        let staged = (|| -> Result<(), String> {
            std::fs::write(
                staging.join("entry.json"),
                serde_json::to_vec_pretty(&manifest_entry).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            if let Some(bytes) = &thumb {
                std::fs::write(staging.join("thumb.png"), bytes).map_err(|e| e.to_string())?;
            }
            Ok(())
        })();
        if let Err(e) = staged {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(e);
        }
        let backup = self.root.join(format!(".bak-{}", entry.id));
        let _ = std::fs::remove_dir_all(&backup);
        let had_old = dir.exists();
        if had_old {
            std::fs::rename(&dir, &backup).map_err(|e| e.to_string())?;
        }
        if let Err(e) = std::fs::rename(&staging, &dir) {
            // Swap failed — put the old entry back and report honestly.
            if had_old {
                let _ = std::fs::rename(&backup, &dir);
            }
            let _ = std::fs::remove_dir_all(&staging);
            return Err(e.to_string());
        }
        if had_old {
            let _ = std::fs::remove_dir_all(&backup);
        }
        Ok(PresetEntryDto {
            id: entry.id,
            preset_type: entry.preset_type,
            schema_version: entry.schema_version,
            meta,
            payload_json: manifest_entry.payload.to_string(),
            has_thumb: thumb.is_some(),
        })
    }

    pub fn delete(&self, entry_id: &str) -> Result<(), String> {
        let _guard = self.write_lock.lock().map_err(|_| "preset store lock poisoned".to_string())?;
        let dir = self.entry_dir(entry_id)?;
        if !dir.exists() {
            return Ok(()); // idempotent
        }
        std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())
    }

    pub fn rename(&self, entry_id: &str, name: &str) -> Result<PresetEntryDto, String> {
        let _guard = self.write_lock.lock().map_err(|_| "preset store lock poisoned".to_string())?;
        let dir = self.entry_dir(entry_id)?;
        let raw = std::fs::read(dir.join("entry.json")).map_err(|e| e.to_string())?;
        let mut entry: ManifestEntry = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
        entry.meta.name = cap_str(name, MAX_NAME).ok_or_else(|| "empty name".to_string())?;
        // Temp-file + rename (codex FIX-6 #6): a crash mid-write must never leave
        // truncated JSON where the lister expects an entry. `std::fs::rename`
        // replaces an existing FILE destination on every platform (Windows uses
        // MOVEFILE_REPLACE_EXISTING), so this is not the dir-rename Windows trap.
        let tmp = dir.join(".entry.json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(&entry).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, dir.join("entry.json")).map_err(|e| e.to_string())?;
        self.load_entry(&dir)
    }

    /// The `dmpreset://<entryId>` thumbnail bytes (library entries only).
    pub fn thumb_for(&self, entry_id: &str) -> Option<Vec<u8>> {
        let dir = self.entry_dir(entry_id).ok()?;
        let bytes = std::fs::read(dir.join("thumb.png")).ok()?;
        bytes.starts_with(&PNG_MAGIC).then_some(bytes)
    }

    // ---- export -------------------------------------------------------------

    /// Build a `.dmpreset` at `dest_path` from the given entries (the current
    /// recipe, or library entries — same shape). Atomic + non-clobbering:
    /// `create_new`, no silent overwrite; the caller's save dialog already
    /// resolved intent, so an existing file is an honest error.
    pub fn export(&self, dest_path: &str, entries: Vec<PresetSaveDto>, now_iso: String) -> Result<String, String> {
        if entries.is_empty() {
            return Err("nothing to export".into());
        }
        if entries.len() > MAX_ENTRIES {
            return Err("too many entries".into());
        }
        let dest = Path::new(dest_path);
        if dest.file_name().is_none() {
            return Err("invalid destination".into());
        }
        let mut manifest_entries = Vec::new();
        let mut thumbs: Vec<(String, Vec<u8>)> = Vec::new();
        for e in entries {
            if !valid_id(&e.id) {
                return Err("invalid entry id".into());
            }
            if e.payload_json.chars().count() > MAX_PAYLOAD_CHARS {
                return Err("payload too large".into());
            }
            let payload: serde_json::Value =
                serde_json::from_str(&e.payload_json).map_err(|err| format!("payload not JSON: {err}"))?;
            let meta = sanitize_meta(&MetaJson {
                name: e.meta.name.clone(),
                author: e.meta.author.clone(),
                description: e.meta.description.clone(),
                created_at: e.meta.created_at.clone(),
            })?;
            let thumb = decode_thumb(e.thumb_png_base64.as_deref())?;
            let thumb_name = thumb.as_ref().map(|_| format!("assets/{}/thumb.png", e.id));
            if let (Some(name), Some(bytes)) = (&thumb_name, thumb) {
                thumbs.push((name.clone(), bytes));
            }
            manifest_entries.push(ManifestEntry {
                id: e.id,
                preset_type: e.preset_type,
                schema_version: e.schema_version,
                meta: MetaJson {
                    name: meta.name,
                    author: meta.author,
                    description: meta.description,
                    created_at: meta.created_at.or_else(|| Some(now_iso.clone())),
                },
                payload,
                thumbnail: thumb_name,
            });
        }
        let manifest = Manifest {
            format: FORMAT.into(),
            generator: format!("DeskMakeover {}", env!("CARGO_PKG_VERSION")),
            created_at: now_iso,
            entries: manifest_entries,
        };

        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(dest)
            .map_err(|e| format!("cannot create {}: {e}", dest.display()))?;
        let mut writer = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        (|| -> Result<(), String> {
            use std::io::Write;
            writer.start_file("manifest.json", opts).map_err(|e| e.to_string())?;
            writer
                .write_all(&serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            for (name, bytes) in &thumbs {
                writer.start_file(name.as_str(), opts).map_err(|e| e.to_string())?;
                writer.write_all(bytes).map_err(|e| e.to_string())?;
            }
            writer.finish().map_err(|e| e.to_string())?;
            Ok(())
        })()
        .inspect_err(|_| {
            // A half-written export must not survive under the claimed name.
            let _ = std::fs::remove_file(dest);
        })?;
        Ok(dest.display().to_string())
    }

    /// Resolve an entry's library dir, refusing ids that could escape the root
    /// (defense in depth on top of `valid_id`'s charset).
    fn entry_dir(&self, entry_id: &str) -> Result<PathBuf, String> {
        if !valid_id(entry_id) {
            return Err("invalid entry id".into());
        }
        let dir = self.root.join(entry_id);
        // valid_id admits no separators, so the join CANNOT escape; assert anyway.
        if !dir.starts_with(&self.root) {
            return Err("path escape".into());
        }
        Ok(dir)
    }
}

// ---- helpers ----------------------------------------------------------------

/// Reject the container when ANY central-directory entry is hostile (spec 09
/// §4.1/§4.2, codex FIX-6 #5) — names with traversal/absolutes/drive letters/
/// NUL/backslashes, declared sizes past the caps or the aggregate budget,
/// compression ratios past 200:1, nested-archive extensions, or compression
/// methods other than Stored/Deflated. Nothing is decompressed here — this
/// reads central-directory metadata only.
fn screen_archive<R: Read + std::io::Seek>(zip: &mut zip::ZipArchive<R>) -> Result<(), String> {
    let mut total_declared: u64 = 0;
    for i in 0..zip.len() {
        let entry = zip.by_index_raw(i).map_err(|e| format!("archive entry {i}: {e}"))?;
        let name = entry.name();
        let lower = name.to_ascii_lowercase();
        let is_dir = name.ends_with('/');
        if name.contains("..")
            || name.contains('\\')
            || name.contains('\0')
            || name.contains(':')
            || name.starts_with('/')
            || (!is_dir && name != "manifest.json" && !safe_asset_name(name))
        {
            return Err(format!("unsafe archive entry name {:?}", sanitize_str(name, 64)));
        }
        if lower.ends_with(".zip")
            || lower.ends_with(".dmpreset")
            || lower.ends_with(".7z")
            || lower.ends_with(".rar")
            || lower.ends_with(".gz")
            || lower.ends_with(".tar")
        {
            return Err("nested archives are not allowed".into());
        }
        match entry.compression() {
            zip::CompressionMethod::Stored | zip::CompressionMethod::Deflated => {}
            other => return Err(format!("unsupported compression method {other}")),
        }
        let declared = entry.size();
        let compressed = entry.compressed_size().max(1);
        if declared > MAX_TOTAL_DECOMPRESSED {
            return Err(format!("{name}: declared size out of range"));
        }
        if declared / compressed > MAX_COMPRESSION_RATIO {
            return Err(format!("{name}: compression ratio out of range"));
        }
        total_declared = total_declared.saturating_add(declared);
        if total_declared > MAX_TOTAL_DECOMPRESSED {
            return Err("declared decompressed size exceeds the budget".into());
        }
    }
    Ok(())
}

/// Running decompression budget across every read from one archive (§4.2).
struct Budget {
    left: u64,
}

impl Budget {
    fn new(total: u64) -> Self {
        Self { left: total }
    }
    fn take(&mut self, n: u64) -> Result<(), String> {
        if n > self.left {
            return Err("decompression budget exceeded".into());
        }
        self.left -= n;
        Ok(())
    }
}

/// Read one named entry with per-entry + global caps and a compression-ratio
/// guard. `Ok(None)` = entry absent. Reads by NAME into memory — nothing is
/// ever extracted to a path, so zip-slip has no surface here; names are
/// additionally screened by `safe_asset_name` before lookup.
fn read_entry_bounded<R: Read + std::io::Seek>(
    zip: &mut zip::ZipArchive<R>,
    name: &str,
    cap: u64,
    budget: &mut Budget,
) -> Result<Option<Vec<u8>>, String> {
    let entry = match zip.by_name(name) {
        Ok(e) => e,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(e) => return Err(format!("{name}: {e}")),
    };
    let declared = entry.size();
    if declared > cap {
        return Err(format!("{name}: too large ({declared} bytes)"));
    }
    let compressed = entry.compressed_size().max(1);
    if declared / compressed > MAX_COMPRESSION_RATIO {
        return Err(format!("{name}: compression ratio out of range"));
    }
    budget.take(declared)?;
    // The declared size can lie; the reader is capped one byte past it so a
    // lying stream trips the check instead of ballooning memory.
    let mut bytes = Vec::new();
    let read = entry
        .take(declared + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("{name}: {e}"))?;
    if read as u64 > declared {
        return Err(format!("{name}: size field lies"));
    }
    if bytes.starts_with(&ZIP_MAGIC) {
        return Err(format!("{name}: nested archives are not allowed"));
    }
    Ok(Some(bytes))
}

/// Entry ids: webview `crypto.randomUUID()` or similar — hyphenated alnum only,
/// so an id can never carry a path.
fn valid_id(id: &str) -> bool {
    (8..=64).contains(&id.len()) && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Asset names referenced by a manifest must be plain relative POSIX paths
/// under assets/ — no traversal, no absolutes, no drive letters, no NUL.
fn safe_asset_name(name: &str) -> bool {
    name.starts_with("assets/")
        && !name.contains("..")
        && !name.contains('\\')
        && !name.contains('\0')
        && !name.contains(':')
        && name.split('/').all(|seg| !seg.is_empty())
}

/// Strip control characters and cap length in CHARS; None when empty after trim.
fn cap_str(s: &str, max: usize) -> Option<String> {
    let cleaned: String = s.chars().filter(|c| !c.is_control()).take(max).collect();
    let trimmed = cleaned.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn sanitize_str(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(max).collect()
}

fn sanitize_meta(meta: &MetaJson) -> Result<PresetMetaDto, String> {
    let name = cap_str(&meta.name, MAX_NAME).ok_or_else(|| "missing name".to_string())?;
    Ok(PresetMetaDto {
        name,
        author: meta.author.as_deref().and_then(|s| cap_str(s, MAX_AUTHOR)),
        description: meta.description.as_deref().and_then(|s| cap_str(s, MAX_DESCRIPTION)),
        created_at: meta.created_at.as_deref().and_then(|s| cap_str(s, 40)),
    })
}

/// Thumbnail pixel budget — thumbs are small preview strips, never wallpapers.
const MAX_THUMB_PIXELS: u64 = 4_000_000;

/// Decode + sniff + RE-ENCODE an inline thumb (save/export input). PNG only in
/// v1 (spec 09 §4.4). Order is the security pipeline (codex FIX-6 #4): magic →
/// IHDR header dimensions (BEFORE any pixel allocation) → full decode → a
/// fresh canonical PNG from our own encoder — original bytes never persist and
/// never reach the webview. None passes through; junk is an error (the caller
/// authored it — unlike a package read, where a bad thumb just drops).
fn decode_thumb(b64: Option<&str>) -> Result<Option<Vec<u8>>, String> {
    let Some(b64) = b64 else { return Ok(None) };
    if b64.len() as u64 > MAX_THUMB_BYTES * 2 {
        return Err("thumb too large".into());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .map_err(|e| format!("thumb: bad base64: {e}"))?;
    reencode_thumb_png(&bytes).map(Some)
}

/// The bounded PNG pipeline shared by save/export input AND package reads.
fn reencode_thumb_png(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if bytes.len() as u64 > MAX_THUMB_BYTES {
        return Err("thumb too large".into());
    }
    if !bytes.starts_with(&PNG_MAGIC) {
        return Err("thumb: not a PNG".into());
    }
    // IHDR is mandatory-first: width/height sit at fixed offsets 16..24. Check
    // the CLAIM before the decoder allocates anything.
    if bytes.len() < 24 {
        return Err("thumb: truncated".into());
    }
    let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]) as u64;
    let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]) as u64;
    if w == 0 || h == 0 || w * h > MAX_THUMB_PIXELS {
        return Err("thumb: dimensions out of range".into());
    }
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .map_err(|e| format!("thumb: not a decodable PNG: {e}"))?;
    // The decoder's own truth must agree with the header claim.
    if (img.width() as u64) * (img.height() as u64) > MAX_THUMB_PIXELS {
        return Err("thumb: dimensions out of range".into());
    }
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .map_err(|e| format!("thumb: re-encode failed: {e}"))?;
    if out.len() as u64 > MAX_THUMB_BYTES {
        return Err("thumb too large".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn store() -> (tempfile::TempDir, PresetStore) {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = PresetStore::new(dir.path());
        (dir, store)
    }

    fn save_dto(id: &str, name: &str) -> PresetSaveDto {
        PresetSaveDto {
            id: id.into(),
            preset_type: "icon".into(),
            schema_version: 1,
            meta: PresetMetaDto {
                name: name.into(),
                author: Some("tester".into()),
                description: None,
                created_at: Some("2026-07-15T00:00:00Z".into()),
            },
            payload_json: r#"{"v":1,"config":{"shape":"Apple"}}"#.into(),
            thumb_png_base64: None,
        }
    }

    fn write_zip(path: &Path, files: &[(&str, &[u8])]) {
        let f = std::fs::File::create(path).expect("create zip");
        let mut w = zip::ZipWriter::new(f);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, bytes) in files {
            w.start_file(*name, opts).expect("start");
            w.write_all(bytes).expect("write");
        }
        w.finish().expect("finish");
    }

    fn manifest_json(entries: &str) -> String {
        format!(
            r#"{{"format":"dmpreset/1","generator":"t","createdAt":"2026-07-15T00:00:00Z","entries":[{entries}]}}"#
        )
    }

    fn icon_entry(id: &str, extra: &str) -> String {
        format!(
            r#"{{"id":"{id}","type":"icon","schemaVersion":1,"meta":{{"name":"暖阳"}},"payload":{{"v":1}}{extra}}}"#
        )
    }

    #[test]
    fn save_list_rename_delete_round_trip() {
        let (_tmp, store) = store();
        let saved = store.save(save_dto("aaaaaaaa-1111", "暖阳陶土"), false).expect("save");
        assert_eq!(saved.meta.name, "暖阳陶土");
        assert!(!saved.has_thumb);
        let listed = store.list();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, "aaaaaaaa-1111");
        let renamed = store.rename("aaaaaaaa-1111", "冷杉").expect("rename");
        assert_eq!(renamed.meta.name, "冷杉");
        store.delete("aaaaaaaa-1111").expect("delete");
        assert!(store.list().is_empty());
        store.delete("aaaaaaaa-1111").expect("idempotent delete");
    }

    #[test]
    fn save_never_clobbers_without_overwrite() {
        let (_tmp, store) = store();
        store.save(save_dto("aaaaaaaa-1111", "one"), false).expect("save");
        let err = store.save(save_dto("aaaaaaaa-1111", "two"), false).unwrap_err();
        assert_eq!(err, "exists");
        store.save(save_dto("aaaaaaaa-1111", "two"), true).expect("overwrite ok");
        assert_eq!(store.list()[0].meta.name, "two");
    }

    #[test]
    fn save_rejects_bad_ids_types_and_meta() {
        let (_tmp, store) = store();
        assert!(store.save(save_dto("../escape-me", "x"), false).is_err());
        assert!(store.save(save_dto("short", "x"), false).is_err());
        let mut wrong_type = save_dto("aaaaaaaa-1111", "x");
        wrong_type.preset_type = "wallpaper".into();
        assert!(store.save(wrong_type, false).is_err());
        let mut empty_name = save_dto("aaaaaaaa-1111", "   ");
        empty_name.meta.name = "   ".into();
        assert!(store.save(empty_name, false).is_err());
        let mut junk_payload = save_dto("aaaaaaaa-1111", "x");
        junk_payload.payload_json = "not json".into();
        assert!(store.save(junk_payload, false).is_err());
    }

    #[test]
    fn strings_are_capped_and_control_chars_stripped() {
        let (_tmp, store) = store();
        let mut dto = save_dto("aaaaaaaa-1111", &"名".repeat(300));
        dto.meta.description = Some(format!("a\u{0007}{}", "b".repeat(900)));
        let saved = store.save(dto, false).expect("save");
        assert_eq!(saved.meta.name.chars().count(), MAX_NAME);
        let desc = saved.meta.description.expect("desc");
        assert!(desc.chars().count() <= MAX_DESCRIPTION);
        assert!(!desc.contains('\u{0007}'));
    }

    #[test]
    fn export_then_read_round_trips_and_never_overwrites() {
        let (tmp, store) = store();
        let dest = tmp.path().join("out.dmpreset");
        let path = store
            .export(dest.to_str().unwrap(), vec![save_dto("aaaaaaaa-1111", "暖阳陶土")], "2026-07-15T00:00:00Z".into())
            .expect("export");
        let read = store.read_package(&path);
        assert!(read.format_ok, "err={:?}", read.error);
        assert_eq!(read.entries.len(), 1);
        let entry = read.entries[0].entry.as_ref().expect("entry");
        assert_eq!(entry.meta.name, "暖阳陶土");
        assert_eq!(entry.preset_type, "icon");
        // Non-clobbering: the same dest fails, no silent replace.
        assert!(store
            .export(dest.to_str().unwrap(), vec![save_dto("aaaaaaaa-1111", "x")], "t".into())
            .is_err());
    }

    #[test]
    fn read_rejects_wrong_format_major_fail_closed() {
        let (tmp, store) = store();
        let p = tmp.path().join("v9.dmpreset");
        write_zip(&p, &[("manifest.json", br#"{"format":"dmpreset/9","generator":"t","createdAt":"t","entries":[]}"# as &[u8])]);
        let read = store.read_package(p.to_str().unwrap());
        assert!(!read.format_ok);
        assert!(read.error.unwrap().contains("update DeskMakeover"));
    }

    #[test]
    fn read_reports_partial_success_per_entry() {
        let (tmp, store) = store();
        let p = tmp.path().join("mixed.dmpreset");
        let entries = [
            icon_entry("aaaaaaaa-1111", ""),
            r#"{"id":"bbbbbbbb-2222","type":"wallpaper","schemaVersion":1,"meta":{"name":"未来"},"payload":{}}"#.to_string(),
            r#"{"id":"!!","type":"icon","schemaVersion":1,"meta":{"name":"bad id"},"payload":{}}"#.to_string(),
        ]
        .join(",");
        write_zip(&p, &[("manifest.json", manifest_json(&entries).as_bytes())]);
        let read = store.read_package(p.to_str().unwrap());
        assert!(read.format_ok);
        assert_eq!(read.entries.len(), 3);
        assert!(read.entries[0].entry.is_some());
        assert!(read.entries[1].entry.is_none()); // reserved type → honest per-entry error
        assert!(read.entries[2].entry.is_none()); // bad id
    }

    #[test]
    fn read_refuses_traversal_thumbnails_but_keeps_the_entry() {
        let (tmp, store) = store();
        let p = tmp.path().join("slip.dmpreset");
        let entry = icon_entry("aaaaaaaa-1111", r#","thumbnail":"../../etc/passwd""#);
        write_zip(&p, &[("manifest.json", manifest_json(&entry).as_bytes())]);
        let read = store.read_package(p.to_str().unwrap());
        let e = &read.entries[0];
        assert!(e.entry.is_some());
        assert!(e.thumb_png_base64.is_none());
        assert!(!e.entry.as_ref().unwrap().has_thumb);
    }

    #[test]
    fn read_rejects_nested_archives_and_oversize_manifests() {
        let (tmp, store) = store();
        // Nested archive masquerading as the manifest.
        let p = tmp.path().join("nested.dmpreset");
        write_zip(&p, &[("manifest.json", &ZIP_MAGIC as &[u8])]);
        let read = store.read_package(p.to_str().unwrap());
        assert!(!read.format_ok);
        // Manifest bigger than its cap.
        let p2 = tmp.path().join("big.dmpreset");
        let big = vec![b' '; (MAX_MANIFEST_BYTES + 10) as usize];
        write_zip(&p2, &[("manifest.json", big.as_slice())]);
        let read2 = store.read_package(p2.to_str().unwrap());
        assert!(!read2.format_ok);
    }

    #[test]
    fn any_hostile_archive_entry_rejects_the_whole_container() {
        let (tmp, store) = store();
        // A traversal-named entry ANYWHERE in the archive (even unreferenced).
        let p = tmp.path().join("slip2.dmpreset");
        write_zip(&p, &[
            ("manifest.json", manifest_json(&icon_entry("aaaaaaaa-1111", "")).as_bytes()),
            ("assets/x/../../evil.png", b"x" as &[u8]),
        ]);
        assert!(!store.read_package(p.to_str().unwrap()).format_ok);
        // A stray non-asset entry name is fail-closed too (layout == library layout).
        let p2 = tmp.path().join("stray.dmpreset");
        write_zip(&p2, &[
            ("manifest.json", manifest_json(&icon_entry("aaaaaaaa-1111", "")).as_bytes()),
            ("evil.txt", b"junk" as &[u8]),
        ]);
        assert!(!store.read_package(p2.to_str().unwrap()).format_ok);
        // A nested-archive extension rejects even when unreferenced.
        let p3 = tmp.path().join("nested2.dmpreset");
        write_zip(&p3, &[
            ("manifest.json", manifest_json(&icon_entry("aaaaaaaa-1111", "")).as_bytes()),
            ("assets/aaaaaaaa-1111/inner.zip", b"PK" as &[u8]),
        ]);
        assert!(!store.read_package(p3.to_str().unwrap()).format_ok);
    }

    #[test]
    fn overwrite_failure_can_never_lose_the_old_entry() {
        let (_tmp, store) = store();
        store.save(save_dto("aaaaaaaa-1111", "keeper"), false).expect("save");
        // A junk-payload overwrite fails BEFORE any swap — the old entry survives.
        let mut bad = save_dto("aaaaaaaa-1111", "usurper");
        bad.payload_json = "not json".into();
        assert!(store.save(bad, true).is_err());
        let listed = store.list();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].meta.name, "keeper");
        // And the swap machinery never leaks .tmp-/.bak- dirs into the listing.
        store.save(save_dto("aaaaaaaa-1111", "usurper"), true).expect("overwrite");
        let after = store.list();
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].meta.name, "usurper");
    }

    #[test]
    fn thumbs_are_reencoded_never_stored_verbatim() {
        let (_tmp, store) = store();
        let mut png = Vec::new();
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 111, 94, 255])))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("encode png");
        // Append trailing junk after IEND — decoders tolerate it, but OUR stored
        // bytes must be the canonical re-encode, not the original stream.
        let mut tainted = png.clone();
        tainted.extend_from_slice(b"JUNKJUNKJUNK");
        let mut dto = save_dto("aaaaaaaa-1111", "thumbed");
        dto.thumb_png_base64 = Some(base64::engine::general_purpose::STANDARD.encode(&tainted));
        store.save(dto, false).expect("save");
        let stored = store.thumb_for("aaaaaaaa-1111").expect("thumb");
        assert!(stored.starts_with(&PNG_MAGIC));
        assert!(!stored.windows(4).any(|w| w == b"JUNK"), "verbatim hostile bytes persisted");
        // An IHDR claiming absurd dimensions dies before decode.
        let mut bomb = png.clone();
        bomb[16..20].copy_from_slice(&50_000u32.to_be_bytes());
        bomb[20..24].copy_from_slice(&50_000u32.to_be_bytes());
        let mut dto2 = save_dto("bbbbbbbb-2222", "bomb");
        dto2.thumb_png_base64 = Some(base64::engine::general_purpose::STANDARD.encode(&bomb));
        assert!(store.save(dto2, false).is_err());
    }

    #[test]
    fn recovery_restores_an_orphaned_backup_from_an_interrupted_swap() {
        let (tmp, _) = store();
        let root = tmp.path().join("presets");
        // Simulate a crash mid-swap: the old entry was parked at .bak-<id> and
        // the canonical dir never got the replacement.
        let bak = root.join(".bak-aaaaaaaa-1111");
        std::fs::create_dir_all(&bak).unwrap();
        std::fs::write(
            bak.join("entry.json"),
            br#"{"id":"aaaaaaaa-1111","type":"icon","schemaVersion":1,"meta":{"name":"survivor"},"payload":{"v":1,"config":{"shape":"Apple"}}}"#,
        )
        .unwrap();
        // Also a stale staging dir that must be swept.
        std::fs::create_dir_all(root.join(".tmp-bbbbbbbb-2222")).unwrap();
        // A fresh store runs recovery in new().
        let recovered = PresetStore::new(tmp.path());
        let listed = recovered.list();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, "aaaaaaaa-1111");
        assert_eq!(listed[0].meta.name, "survivor");
        assert!(!root.join(".bak-aaaaaaaa-1111").exists());
        assert!(!root.join(".tmp-bbbbbbbb-2222").exists());
    }

    #[test]
    fn recovery_drops_a_superseded_backup_when_the_canonical_survived() {
        let (tmp, store) = store();
        store.save(save_dto("aaaaaaaa-1111", "current"), false).expect("save");
        // A .bak left over AFTER a successful swap (crash before backup cleanup):
        // the canonical exists, so recovery drops the stale backup, keeps current.
        let bak = tmp.path().join("presets").join(".bak-aaaaaaaa-1111");
        std::fs::create_dir_all(&bak).unwrap();
        std::fs::write(bak.join("entry.json"), b"stale").unwrap();
        let recovered = PresetStore::new(tmp.path());
        let listed = recovered.list();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].meta.name, "current");
        assert!(!bak.exists());
    }

    #[test]
    fn read_ignores_a_non_dmpreset_file_gracefully() {
        let (tmp, store) = store();
        let p = tmp.path().join("junk.dmpreset");
        std::fs::write(&p, b"this is not a zip").unwrap();
        let read = store.read_package(p.to_str().unwrap());
        assert!(!read.format_ok);
        assert!(read.error.is_some());
    }

    #[test]
    fn thumbs_round_trip_through_save_and_protocol() {
        let (_tmp, store) = store();
        // A real PNG, encoded by the same codec the validator decodes with.
        let mut png = Vec::new();
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 111, 94, 255])))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("encode png");
        let mut dto = save_dto("aaaaaaaa-1111", "with thumb");
        dto.thumb_png_base64 = Some(base64::engine::general_purpose::STANDARD.encode(&png));
        let saved = store.save(dto, false).expect("save");
        assert!(saved.has_thumb);
        let bytes = store.thumb_for("aaaaaaaa-1111").expect("thumb bytes");
        assert!(bytes.starts_with(&PNG_MAGIC));
        assert!(store.thumb_for("../../aaaaaaaa-1111").is_none());
    }
}
