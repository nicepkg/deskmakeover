//! The production content-addressed asset store (spec 07 §5, Wave B B7).
//!
//! Generated `.ico` bytes live on disk under one app-data directory, addressed by the caller's
//! content hash (`<source-hash>-<style-hash>`, produced by the icon core). This is the ONLY real
//! [`dm_domain::AssetStore`] — every other impl is a test fake (`txn/fakes.rs`). Writes are
//! crash-atomic and idempotent; [`FsAssetStore::gc`] is scoped so it can only ever delete
//! DeskMakeover's own generated assets, never a user file (the ADR-0020 data-loss red-line).

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use dm_domain::{AssetRef, AssetStore, PortError, PortResult};

use crate::fs_atomic::write_atomic;

/// The on-disk extension every generated asset carries.
const ASSET_EXT: &str = "ico";

/// A filesystem-backed content-addressed store rooted at one directory (typically
/// `<app-data>/assets`). The directory is created lazily on the first write.
pub struct FsAssetStore {
    root: PathBuf,
}

impl FsAssetStore {
    /// Roots the store at `dir`. The directory need not exist yet — it is created on first write.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { root: dir.into() }
    }

    /// The directory this store owns.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The on-disk path for a content hash (`<root>/<hash>.ico`), or [`PortError::Io`] if `hash`
    /// is not a safe single-segment filename — defense-in-depth so a malformed caller-produced
    /// hash can never escape `root` via a separator or `..`.
    fn asset_path(&self, hash: &str) -> PortResult<PathBuf> {
        if !is_safe_hash(hash) {
            return Err(PortError::Io(format!("unsafe asset hash {hash:?}")));
        }
        Ok(self.root.join(format!("{hash}.{ASSET_EXT}")))
    }

    /// Materializes `bytes` under `hash`, returning its reference. Idempotent + crash-atomic, but
    /// "already present" is NOT blindly trusted: content-addressing only holds for a REGULAR file
    /// WE wrote atomically, so the store reuses the existing file only when it is a regular file
    /// whose bytes already equal `bytes`. A symlink, directory, zero-byte/torn, or wrong-content
    /// file at the path is atomically overwritten (the rename replaces a symlink rather than
    /// following it), never silently reused.
    fn write(&self, hash: &str, bytes: &[u8]) -> PortResult<AssetRef> {
        self.ensure_root_not_symlink()?;
        let path = self.asset_path(hash)?;
        if !is_regular_file_with(&path, bytes)? {
            write_atomic(&path, bytes).map_err(|e| PortError::Io(e.to_string()))?;
        }
        Ok(AssetRef::new(hash, path.to_string_lossy().into_owned()))
    }

    /// The ownership boundary (ADR-0020 data-loss red-line): refuse to write to / collect through a
    /// symlinked root, so a mispointed or hijacked root (e.g. `assets -> ~/Pictures`) can never let
    /// the store touch files outside its own app-data directory. A missing root is fine (a write
    /// creates it). [WINDOWS-VERIFY] a directory junction is a reparse point `is_symlink` may not
    /// flag — the Windows adapter must also reject `FILE_ATTRIBUTE_REPARSE_POINT`. An ancestor
    /// symlink is out of scope (it requires write access to the OS app-data tree, i.e. the user is
    /// already fully compromised).
    fn ensure_root_not_symlink(&self) -> PortResult<()> {
        match fs::symlink_metadata(&self.root) {
            Ok(meta) if meta.file_type().is_symlink() => Err(PortError::Io(format!(
                "refusing to operate through a symlinked asset root: {}",
                self.root.display()
            ))),
            _ => Ok(()),
        }
    }
}

impl AssetStore for FsAssetStore {
    fn put(&self, hash: &str, bytes: &[u8]) -> PortResult<AssetRef> {
        // "-empty" is the reserved suffix for paired empty variants (put_empty_variant). A primary
        // hash ending in it would collide with another asset's empty file, so reject it outright —
        // a real content hash never ends this way. Compared case-insensitively so a mixed-case
        // "abc-EMPTY" can't slip past on a case-insensitive filesystem (Windows / default macOS).
        if hash.to_ascii_lowercase().ends_with("-empty") {
            return Err(PortError::Io(format!(
                "asset hash {hash:?} uses the reserved \"-empty\" suffix"
            )));
        }
        self.write(hash, bytes)
    }

    fn put_empty_variant(&self, primary: &AssetRef, bytes: &[u8]) -> PortResult<AssetRef> {
        // The paired empty asset is addressed relative to the primary by the store convention
        // `<primary-hash>-empty` (matching `txn/fakes.rs`), so the applier references the EXACT ref
        // the driver materialized, never a guessed unwritten path (P1-14).
        self.write(&format!("{}-empty", primary.hash), bytes)
    }

    fn exists(&self, asset: &AssetRef) -> PortResult<bool> {
        Ok(Path::new(&asset.path).is_file())
    }

    fn gc(&self, live: &[String]) -> PortResult<()> {
        // Ownership boundary (ADR-0020 data-loss red-line): refuse to collect THROUGH a symlinked
        // root, so a mispointed/hijacked root can never let gc delete files outside the store's own
        // app-data directory.
        self.ensure_root_not_symlink()?;
        let live: HashSet<&str> = live.iter().map(|s| s.as_str()).collect();
        let entries = match fs::read_dir(&self.root) {
            Ok(e) => e,
            // A store directory that was never created has nothing to collect.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(PortError::Io(e.to_string())),
        };
        for entry in entries {
            let entry = entry.map_err(|e| PortError::Io(e.to_string()))?;
            let path = entry.path();
            // The data-loss red-line (ADR-0020): only ever delete a REGULAR `*.ico` file sitting
            // directly in `root` whose stem is an asset hash NOT in `live`. Never recurse, never
            // follow a directory/symlink, never touch a non-`.ico` file — gc can only remove
            // DeskMakeover's own generated assets. `file_type()` does not traverse symlinks, so a
            // symlink is reported as its own kind (not a file) and is skipped.
            if !entry.file_type().map_err(|e| PortError::Io(e.to_string()))?.is_file() {
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some(ASSET_EXT) {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if !live.contains(stem) {
                fs::remove_file(&path).map_err(|e| PortError::Io(e.to_string()))?;
            }
        }
        Ok(())
    }
}

/// Whether `path` is ALREADY a regular file whose bytes equal `bytes` — the only case the store may
/// reuse rather than rewrite. A symlink, directory, unreadable, or content-mismatched/torn file
/// returns `false` (so it is atomically overwritten), never silently trusted: content-addressing
/// only holds for files the store wrote atomically itself. Uses `symlink_metadata` so a symlink is
/// classified as a symlink (not followed).
fn is_regular_file_with(path: &Path, bytes: &[u8]) -> PortResult<bool> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_file() => {
            // Length-first: a size mismatch means "not equal" without reading the file, so a hostile
            // pre-placed huge/sparse `.ico` can never force an unbounded read. Only a same-length
            // regular file is read for the full byte comparison. (A regular→symlink swap in the
            // window between this stat and the read is a residual local-write TOCTOU, [WINDOWS-VERIFY]
            // / documented; it requires app-data write access — full compromise already.)
            if meta.len() != bytes.len() as u64 {
                return Ok(false);
            }
            match fs::read(path) {
                Ok(existing) => Ok(existing == bytes),
                Err(_) => Ok(false),
            }
        }
        // A symlink or directory sitting at our path: overwrite it with a fresh regular file.
        Ok(_) => Ok(false),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(PortError::Io(e.to_string())),
    }
}

/// Whether `hash` is a safe single-path-segment filename: non-empty and only `[A-Za-z0-9_-]`. A
/// generated hash is hex plus the `-`/`-empty` join convention, so this never rejects a legitimate
/// asset; it exists purely to stop a malformed hash (separators, `..`, a dot sequence) from
/// escaping `root`.
fn is_safe_hash(hash: &str) -> bool {
    !hash.is_empty()
        && hash.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, FsAssetStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = FsAssetStore::new(dir.path().join("assets"));
        (dir, store)
    }

    #[test]
    fn put_materializes_a_file_and_reports_it_exists() {
        let (_dir, store) = store();
        let asset = store.put("abc123", b"ICO-BYTES").unwrap();
        assert_eq!(asset.hash, "abc123");
        assert!(asset.path.ends_with("abc123.ico"));
        assert!(store.exists(&asset).unwrap());
        assert_eq!(fs::read(&asset.path).unwrap(), b"ICO-BYTES");
        // No stray temp file survives the rename.
        assert!(!store.root().join("abc123.ico.tmp").exists());
    }

    #[test]
    fn put_is_idempotent_and_reuses_an_existing_asset() {
        let (_dir, store) = store();
        let first = store.put("hash", b"original").unwrap();
        // A second put with the SAME hash returns the same ref and does not error. Content-
        // addressing means the bytes are identical by construction, so the existing file is reused
        // (the write is skipped) — the on-disk bytes stay as first written.
        let second = store.put("hash", b"original").unwrap();
        assert_eq!(first, second);
        assert_eq!(fs::read(&first.path).unwrap(), b"original");
    }

    #[test]
    fn put_empty_variant_addresses_relative_to_primary() {
        let (_dir, store) = store();
        let primary = store.put("deadbeef", b"full").unwrap();
        let empty = store.put_empty_variant(&primary, b"empty").unwrap();
        assert_eq!(empty.hash, "deadbeef-empty");
        assert!(empty.path.ends_with("deadbeef-empty.ico"));
        assert!(store.exists(&empty).unwrap());
        assert_eq!(fs::read(&empty.path).unwrap(), b"empty");
    }

    #[test]
    fn exists_is_false_for_an_unwritten_ref() {
        let (_dir, store) = store();
        let phantom = AssetRef::new("never", store.root().join("never.ico").to_string_lossy());
        assert!(!store.exists(&phantom).unwrap());
    }

    #[test]
    fn unsafe_hashes_are_rejected_before_touching_the_filesystem() {
        let (_dir, store) = store();
        for bad in ["", "../evil", "a/b", "a\\b", "..", ".", "with space", "dot.ted"] {
            assert!(
                matches!(store.put(bad, b"x"), Err(PortError::Io(_))),
                "hash {bad:?} must be rejected",
            );
        }
        // Nothing escaped the root.
        assert!(!store.root().parent().unwrap().join("evil.ico").exists());
    }

    #[test]
    fn gc_deletes_only_orphaned_assets_and_keeps_the_live_set() {
        let (_dir, store) = store();
        let keep = store.put("keepme", b"a").unwrap();
        let primary = store.put("withempty", b"b").unwrap();
        let empty = store.put_empty_variant(&primary, b"c").unwrap();
        let orphan = store.put("orphan", b"d").unwrap();

        // Live = the primary + its paired empty + one unrelated survivor. The orphan is absent.
        store.gc(&["keepme".into(), "withempty".into(), "withempty-empty".into()]).unwrap();

        assert!(store.exists(&keep).unwrap(), "an explicitly-live asset survives");
        assert!(store.exists(&primary).unwrap(), "a live primary survives");
        assert!(store.exists(&empty).unwrap(), "a live paired-empty survives");
        assert!(!store.exists(&orphan).unwrap(), "an unreferenced asset is collected");
    }

    #[test]
    fn gc_never_touches_non_ico_files_or_subdirectories() {
        let (_dir, store) = store();
        // Seed the root, then drop a user file and a subdirectory into it.
        store.put("real", b"x").unwrap();
        let user_file = store.root().join("notes.txt");
        fs::write(&user_file, b"do not touch").unwrap();
        let subdir = store.root().join("nested");
        fs::create_dir(&subdir).unwrap();
        let nested_ico = subdir.join("deep.ico");
        fs::write(&nested_ico, b"not mine to gc").unwrap();

        // An empty live set would delete every top-level `.ico` — but only those.
        store.gc(&[]).unwrap();

        assert!(!store.root().join("real.ico").exists(), "a top-level orphan is collected");
        assert!(user_file.exists(), "a non-.ico user file is untouched");
        assert!(subdir.exists(), "a subdirectory is untouched");
        assert!(nested_ico.exists(), "gc never recurses into subdirectories");
    }

    #[test]
    fn gc_on_a_never_created_store_is_a_noop() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsAssetStore::new(dir.path().join("does-not-exist-yet"));
        // No directory, nothing to collect — must succeed, not error.
        store.gc(&["anything".into()]).unwrap();
        assert!(!store.root().exists());
    }

    #[test]
    fn put_rewrites_a_preexisting_wrong_or_torn_file_instead_of_trusting_it() {
        let (_dir, store) = store();
        fs::create_dir_all(store.root()).unwrap();
        // A pre-placed zero-byte / wrong-content file at the hash path must NOT be reused.
        let path = store.root().join("h.ico");
        fs::write(&path, b"").unwrap();
        let asset = store.put("h", b"CORRECT-ICO").unwrap();
        assert_eq!(fs::read(&asset.path).unwrap(), b"CORRECT-ICO", "a torn/wrong file is overwritten");
    }

    #[cfg(unix)]
    #[test]
    fn put_overwrites_a_symlink_at_the_target_without_following_it() {
        use std::os::unix::fs::symlink;
        let (_dir, store) = store();
        fs::create_dir_all(store.root()).unwrap();
        let user_file = store.root().parent().unwrap().join("user.dat");
        fs::write(&user_file, b"USER DATA").unwrap();
        // A hostile symlink where our asset would live.
        symlink(&user_file, store.root().join("h.ico")).unwrap();

        let asset = store.put("h", b"OURS").unwrap();

        assert_eq!(fs::read(&user_file).unwrap(), b"USER DATA", "the symlink target is untouched");
        assert_eq!(fs::read(&asset.path).unwrap(), b"OURS", "our path is now a regular file");
        assert!(!fs::symlink_metadata(&asset.path).unwrap().file_type().is_symlink());
    }

    #[test]
    fn put_rejects_the_reserved_empty_suffix_on_a_primary_hash() {
        let (_dir, store) = store();
        assert!(matches!(store.put("abc-empty", b"x"), Err(PortError::Io(_))));
        // Case-insensitive: a mixed-case variant must also be rejected (a case-insensitive FS would
        // otherwise collide `abc-EMPTY.ico` with the empty variant `abc-empty.ico`).
        assert!(matches!(store.put("abc-EMPTY", b"x"), Err(PortError::Io(_))));
        assert!(matches!(store.put("abc-Empty", b"x"), Err(PortError::Io(_))));
        // But put_empty_variant, which OWNS that suffix, still works.
        let primary = store.put("abc", b"full").unwrap();
        assert!(store.put_empty_variant(&primary, b"empty").is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn put_refuses_a_symlinked_root_never_writing_through_it() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("pictures");
        fs::create_dir_all(&real).unwrap();
        fs::write(real.join("wedding.ico"), b"precious").unwrap();
        let root = dir.path().join("assets");
        symlink(&real, &root).unwrap();

        let store = FsAssetStore::new(&root);
        // A write through the symlinked root must be refused, not land inside the user's directory.
        assert!(matches!(store.put("newhash", b"OURS"), Err(PortError::Io(_))));
        assert!(!real.join("newhash.ico").exists(), "nothing was written through the symlink");
        assert!(real.join("wedding.ico").exists());
    }

    #[cfg(unix)]
    #[test]
    fn gc_refuses_a_symlinked_root() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        // A real directory full of the user's files, and a store root symlinked onto it.
        let real = dir.path().join("pictures");
        fs::create_dir_all(&real).unwrap();
        fs::write(real.join("wedding.ico"), b"precious").unwrap();
        let root = dir.path().join("assets");
        symlink(&real, &root).unwrap();

        let store = FsAssetStore::new(&root);
        // gc must refuse rather than delete through the symlink.
        assert!(matches!(store.gc(&[]), Err(PortError::Io(_))));
        assert!(real.join("wedding.ico").exists(), "the user's file survives");
    }
}
