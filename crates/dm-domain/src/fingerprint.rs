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

    /// Parses a lowercase-hex fingerprint; `None` if malformed. Operates on BYTES (audit F11): a
    /// 64-BYTE string can still contain a multi-byte UTF-8 char, and `&hex[i*2..i*2+2]` would panic
    /// slicing mid-codepoint. Non-hex bytes decode to `None`, never a crash.
    pub fn from_hex(hex: &str) -> Option<Self> {
        let bytes = hex.as_bytes();
        if bytes.len() != 64 {
            return None;
        }
        let mut out = [0u8; 32];
        for (i, byte) in out.iter_mut().enumerate() {
            let hi = (bytes[i * 2] as char).to_digit(16)?;
            let lo = (bytes[i * 2 + 1] as char).to_digit(16)?;
            *byte = (hi << 4 | lo) as u8;
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

    #[test]
    fn empty_input_has_a_stable_nonzero_fingerprint() {
        // An empty file (0 bytes) must still fingerprint deterministically — it is a legitimate
        // CAS state, not an error.
        let a = Fingerprint::of_bytes(b"");
        let b = Fingerprint::of_bytes(b"");
        assert_eq!(a, b);
        assert_ne!(a, Fingerprint::of_bytes(b"x"));
        // sha256("") is well-known; confirm the hex is the canonical value.
        assert_eq!(a.to_hex(), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    }

    #[test]
    fn of_parts_distinguishes_empty_part_from_absent_part() {
        // Length-prefixed framing: [] (no parts) must differ from [""] (one empty part).
        assert_ne!(Fingerprint::of_parts(&[]), Fingerprint::of_parts(&[b""]));
        // Two empty parts differ from one empty part.
        assert_ne!(Fingerprint::of_parts(&[b""]), Fingerprint::of_parts(&[b"", b""]));
    }

    #[test]
    fn from_hex_length_boundaries() {
        let valid = Fingerprint::of_bytes(b"x").to_hex();
        assert!(Fingerprint::from_hex(&valid).is_some());
        assert!(Fingerprint::from_hex(&valid[..63]).is_none()); // 63 chars
        assert!(Fingerprint::from_hex(&format!("{valid}0")).is_none()); // 65 chars
        assert!(Fingerprint::from_hex("").is_none());
    }
}
