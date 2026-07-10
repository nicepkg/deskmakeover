//! Content fingerprints — the compare-and-swap anchor for conflict detection.
//!
//! The frozen oracle preflighted each apply by reading the target's current bytes and
//! comparing them to the snapshot's original with `SequenceEqual`
//! (`DeskMakeover.Shell/DesktopIconApplyOperations.cs`). A fingerprint is the durable,
//! compact form of that check: `SHA-256` over the bytes (or a canonical join of the
//! registry values) that identify an item's on-disk state. Equal fingerprints ⇒ unchanged;
//! a mismatch is an external modification (spec 07 §5 conflict, never a silent overwrite).

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A 32-byte content fingerprint, serialized as lowercase hex.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fingerprint([u8; 32]);

impl Fingerprint {
    /// Fingerprints a single byte blob (a `.lnk`/`.url`/`desktop.ini` file's contents).
    pub fn of_bytes(bytes: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        Self(hasher.finalize().into())
    }

    /// Fingerprints an ordered set of parts with an unambiguous framing (each part is
    /// length-prefixed), so registry value-sets fingerprint without delimiter collisions.
    pub fn of_parts(parts: &[&[u8]]) -> Self {
        let mut hasher = Sha256::new();
        for part in parts {
            hasher.update((part.len() as u64).to_le_bytes());
            hasher.update(part);
        }
        Self(hasher.finalize().into())
    }

    /// The lowercase-hex rendering.
    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// Parses a lowercase-hex fingerprint; `None` if malformed.
    pub fn from_hex(hex: &str) -> Option<Self> {
        if hex.len() != 64 {
            return None;
        }
        let mut out = [0u8; 32];
        for (i, byte) in out.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
        }
        Some(Self(out))
    }
}

impl Serialize for Fingerprint {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Fingerprint {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let hex = String::deserialize(d)?;
        Fingerprint::from_hex(&hex)
            .ok_or_else(|| serde::de::Error::custom("malformed fingerprint hex"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_bytes_fingerprint_equal_and_different_bytes_differ() {
        let a = Fingerprint::of_bytes(b"original shortcut");
        let b = Fingerprint::of_bytes(b"original shortcut");
        let c = Fingerprint::of_bytes(b"changed elsewhere");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn parts_framing_avoids_delimiter_collisions() {
        // ["a","bc"] and ["ab","c"] must NOT collide (naive concatenation would).
        let a = Fingerprint::of_parts(&[b"a", b"bc"]);
        let b = Fingerprint::of_parts(&[b"ab", b"c"]);
        assert_ne!(a, b);
    }

    #[test]
    fn hex_round_trips_through_serde() {
        let fp = Fingerprint::of_bytes(b"x");
        let json = serde_json::to_string(&fp).unwrap();
        let back: Fingerprint = serde_json::from_str(&json).unwrap();
        assert_eq!(fp, back);
        assert_eq!(json, format!("\"{}\"", fp.to_hex()));
    }

    #[test]
    fn malformed_hex_is_rejected() {
        assert!(Fingerprint::from_hex("nope").is_none());
        assert!(serde_json::from_str::<Fingerprint>("\"zz\"").is_err());
    }
}
