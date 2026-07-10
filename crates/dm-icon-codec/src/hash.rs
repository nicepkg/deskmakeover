//! Content addressing for generated `.ico` assets.
//!
//! The M34 transaction driver stores the ICO bytes as an opaque, content-addressed
//! `AssetRef` (`assets.put(hash, bytes)`, spec 07 §5): identical frames must produce
//! identical bytes and therefore the same hash, so two applies needing the same asset
//! reuse one file. The content hash is the lowercase hex SHA-256 of the ICO bytes.

use dm_icon_core::raster::Raster;
use sha2::{Digest, Sha256};

use crate::ico::write_ico;

/// A generated `.ico` paired with its content address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IcoAsset {
    pub bytes: Vec<u8>,
    /// Lowercase hex SHA-256 of `bytes` (64 chars).
    pub content_hash: String,
}

/// Lowercase hex SHA-256 of arbitrary bytes — the ICO content address.
pub fn content_hash(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(64);
    for byte in digest {
        // Two lowercase hex nibbles per byte, no allocation per iteration.
        hex.push(nibble(byte >> 4));
        hex.push(nibble(byte & 0x0f));
    }
    hex
}

fn nibble(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        _ => (b'a' + (n - 10)) as char,
    }
}

/// Assemble an ICO from frames and pair it with its content hash.
pub fn write_ico_asset(frames: &[Raster]) -> IcoAsset {
    let bytes = write_ico(frames);
    let content_hash = content_hash(&bytes);
    IcoAsset { bytes, content_hash }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_matches_a_known_sha256_vector() {
        // SHA-256("") and SHA-256("abc") — the canonical NIST test vectors.
        assert_eq!(
            content_hash(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            content_hash(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn identical_frames_hash_identically_and_differ_from_others() {
        let mut a = Raster::new(16, 16);
        a.data.iter_mut().for_each(|v| *v = 200);
        let mut b = Raster::new(16, 16);
        b.data.iter_mut().for_each(|v| *v = 200);
        let mut c = Raster::new(16, 16);
        c.data.iter_mut().for_each(|v| *v = 201);

        let asset_a = write_ico_asset(&[a]);
        let asset_b = write_ico_asset(&[b]);
        let asset_c = write_ico_asset(&[c]);

        assert_eq!(asset_a.content_hash, asset_b.content_hash);
        assert_eq!(asset_a.content_hash.len(), 64);
        assert_ne!(asset_a.content_hash, asset_c.content_hash);
    }
}
