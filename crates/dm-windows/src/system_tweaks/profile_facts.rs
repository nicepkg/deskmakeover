//! Pure assembly of the Windows environment fingerprint from raw gathered fields.
//!
//! [`assemble_environment`] is the SINGLE canonicalization point: the `cfg(windows)` probe in
//! [`super::profile`] does only the raw Win32 gathering (registry reads + `GetProductInfo` +
//! `GetNativeSystemInfo`) into a [`RawProfileFacts`], then hands it here. Every field is
//! canonicalized through the `dm-domain` helpers so the assembled [`WindowsEnvironment`] is either
//! self-consistently certifiable or fails `is_certifiable` closed — the probe NEVER decides
//! support, it only reports faithfully. This split keeps all canonicalization host-tested on Mac;
//! only the raw gathering is `[WINDOWS-VERIFY]`.

use dm_domain::system_tweaks::{RegistrySnapshot, WindowsEdition, WindowsEnvironment};

/// The raw, un-canonicalized fields the platform probe gathers. Strings are exactly as read from
/// the registry / Win32 APIs (untrimmed, original case); [`assemble_environment`] does all
/// normalization so there is one place to test it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawProfileFacts {
    pub major: u32,
    pub minor: u32,
    pub build: u32,
    pub ubr: u32,
    /// Registry `CurrentVersion\DisplayVersion` (e.g. `24H2`).
    pub display_version: String,
    /// Registry `CurrentVersion\EditionID` (e.g. `Professional`).
    pub edition_id: String,
    /// Registry `CurrentVersion\InstallationType` (e.g. `Client`).
    pub installation_type: String,
    /// `GetProductInfo` SKU (historically "product type").
    pub product_type: u32,
    /// `OSVERSIONINFOEXW.wProductType == VER_NT_WORKSTATION`.
    pub is_workstation: bool,
    /// Registry `HKCU\Control Panel\International\Geo\Name` (ISO 3166 alpha-2).
    pub region: String,
    /// `GetNativeSystemInfo` architecture, mapped to `x64`/`arm64`/`x86`.
    pub native_architecture: String,
    /// This process's architecture (compile-time target), mapped the same way; kept separate so
    /// emulation (an x64 build on arm64) is certified apart from the native machine.
    pub process_architecture: String,
    /// Whether the process runs with package identity (registry virtualization / API surface).
    pub packaged: bool,
}

/// Canonicalize raw facts into a [`WindowsEnvironment`]. Total (never fails): a malformed edition
/// identity, a non-ISO region, or an odd architecture string are stored in a shape that makes
/// [`WindowsEnvironment::is_certifiable`] return `false`, so the engine cleanly surfaces the row as
/// unsupported instead of the whole probe erroring.
pub fn assemble_environment(facts: RawProfileFacts) -> WindowsEnvironment {
    // Edition: prefer the canonical (EditionID, family) the domain accepts — this covers a
    // genuinely-unknown-but-self-consistent edition as `Other`. A one-sided / masquerading identity
    // returns `None`; keep the raw EditionID + an `Other` family so certification fails closed.
    let (edition_id, edition) =
        WindowsEdition::canonical_raw_identity(&facts.edition_id, facts.product_type).unwrap_or_else(
            || {
                let raw = facts.edition_id.trim().to_string();
                (raw.clone(), WindowsEdition::Other(raw.to_ascii_uppercase()))
            },
        );

    // Region: store the canonical form when it is a real ISO alpha-2 / M.49 code, else the raw
    // uppercased (which will not self-match `canonical_region`, so it is not certifiable).
    let region = WindowsEnvironment::canonical_region(&facts.region)
        .unwrap_or_else(|| facts.region.trim().to_ascii_uppercase());

    WindowsEnvironment {
        major: facts.major,
        minor: facts.minor,
        build: facts.build,
        ubr: facts.ubr,
        display_version: facts.display_version.trim().to_ascii_uppercase(),
        edition_id,
        edition,
        installation_type: facts.installation_type.trim().to_string(),
        product_type: facts.product_type,
        is_workstation: facts.is_workstation,
        region,
        native_architecture: facts.native_architecture.trim().to_ascii_lowercase(),
        process_architecture: facts.process_architecture.trim().to_ascii_lowercase(),
        packaged: facts.packaged,
    }
}

// ── Pure extractors for the raw registry reads the probe gathers ─────────────────────────────
// The `cfg(windows)` probe reads each field through `WinregBackend`; these turn the resulting
// `RegistrySnapshot`s into the plain scalars `RawProfileFacts` holds, with the decode logic
// host-tested here rather than behind `[WINDOWS-VERIFY]`.

/// The `u32` a present DWORD snapshot holds, else `None` (a missing value or a non-DWORD/malformed
/// width never reads as a silent zero).
pub fn snapshot_dword(snapshot: &RegistrySnapshot) -> Option<u32> {
    snapshot.value().and_then(|value| value.as_dword())
}

/// The string a present `REG_SZ`/`REG_EXPAND_SZ` snapshot holds (UTF-16LE, NUL-trimmed), else
/// `None` (a missing value or a non-string type is not a string field).
pub fn snapshot_string(snapshot: &RegistrySnapshot) -> Option<String> {
    use dm_domain::system_tweaks::RegistryValueKind::{ExpandString, String as Sz};
    let value = snapshot.value()?;
    match value.kind {
        Sz | ExpandString => Some(decode_utf16le(&value.bytes)),
        _ => None,
    }
}

/// Decode a registry UTF-16LE string blob, stopping at the first NUL terminator. An odd trailing
/// byte is dropped (a well-formed registry string is always an even byte count).
pub fn decode_utf16le(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    let end = units.iter().position(|&unit| unit == 0).unwrap_or(units.len());
    String::from_utf16_lossy(&units[..end])
}

/// Map a `PROCESSOR_ARCHITECTURE` environment string to the canonical architecture the domain
/// certifies. An unrecognized value is passed through lowercased so it fails `is_certifiable`
/// closed rather than masquerading as a supported arch.
pub fn map_processor_architecture(raw: &str) -> String {
    match raw.trim().to_ascii_uppercase().as_str() {
        "AMD64" | "X64" => "x64".to_string(),
        "ARM64" => "arm64".to_string(),
        "X86" => "x86".to_string(),
        other => other.to_ascii_lowercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dm_domain::system_tweaks::{RawRegistryValue, RegistryValueKind};

    fn sz(text: &str) -> RegistrySnapshot {
        let mut bytes: Vec<u8> = text.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        bytes.extend_from_slice(&[0, 0]); // NUL terminator, as the registry stores it
        RegistrySnapshot::Present(RawRegistryValue::new(RegistryValueKind::String, bytes))
    }

    /// A well-formed 24H2 Pro fixture, with a few fields deliberately given in noisy raw form
    /// (lowercase region, padded display version) to prove assembly canonicalizes them.
    fn raw_pro_24h2() -> RawProfileFacts {
        RawProfileFacts {
            major: 10,
            minor: 0,
            build: 26_100,
            ubr: 8_737,
            display_version: " 24h2 ".into(),
            edition_id: "Professional".into(),
            installation_type: "Client".into(),
            product_type: 48,
            is_workstation: true,
            region: "us".into(),
            native_architecture: "X64".into(),
            process_architecture: "X64".into(),
            packaged: false,
        }
    }

    #[test]
    fn a_well_formed_profile_assembles_certifiable() {
        let env = assemble_environment(raw_pro_24h2());
        assert_eq!(env.display_version, "24H2");
        assert_eq!(env.region, "US");
        assert_eq!(env.native_architecture, "x64");
        assert_eq!(env.edition, WindowsEdition::Pro);
        assert!(env.is_windows_11());
        assert!(
            env.is_certifiable(),
            "a faithfully-gathered 24H2 Pro profile must be certifiable"
        );
    }

    #[test]
    fn a_masquerading_edition_identity_assembles_non_certifiable() {
        // Known EditionID string but a SKU that belongs to a different family → canonical identity
        // is None → not certifiable, but assembly still returns an environment (clean "unsupported"
        // rather than a hard probe error).
        let mut raw = raw_pro_24h2();
        raw.product_type = 4; // Enterprise SKU under a "Professional" EditionID
        let env = assemble_environment(raw);
        assert!(matches!(env.edition, WindowsEdition::Other(_)));
        assert!(!env.is_certifiable());
    }

    #[test]
    fn a_genuinely_unknown_but_consistent_edition_is_other_shaped() {
        let mut raw = raw_pro_24h2();
        raw.edition_id = "ServerCore".into();
        raw.product_type = 500; // both sides unknown → a self-consistent Other
        let env = assemble_environment(raw);
        assert_eq!(env.edition, WindowsEdition::Other("SERVERCORE".into()));
        assert_eq!(env.edition_id, "SERVERCORE");
    }

    #[test]
    fn a_non_iso_region_fails_closed_but_still_assembles() {
        let mut raw = raw_pro_24h2();
        raw.region = "ZZ".into();
        let env = assemble_environment(raw);
        assert_eq!(env.region, "ZZ");
        assert!(!env.is_certifiable());
    }

    #[test]
    fn an_x86_native_arch_is_stored_lowercased_and_not_certifiable() {
        let mut raw = raw_pro_24h2();
        raw.native_architecture = "X86".into();
        let env = assemble_environment(raw);
        assert_eq!(env.native_architecture, "x86");
        assert!(!env.is_certifiable());
    }

    #[test]
    fn a_non_client_install_is_not_certifiable() {
        let mut raw = raw_pro_24h2();
        raw.installation_type = "Server".into();
        assert!(!assemble_environment(raw).is_certifiable());
    }

    #[test]
    fn snapshot_dword_reads_only_a_present_well_formed_dword() {
        assert_eq!(
            snapshot_dword(&RegistrySnapshot::Present(RawRegistryValue::dword(8_737))),
            Some(8_737)
        );
        assert_eq!(snapshot_dword(&RegistrySnapshot::ValueMissing), None);
        assert_eq!(snapshot_dword(&RegistrySnapshot::KeyMissing), None);
        // A string value is not a DWORD.
        assert_eq!(snapshot_dword(&sz("26100")), None);
    }

    #[test]
    fn snapshot_string_decodes_sz_and_rejects_non_strings() {
        assert_eq!(snapshot_string(&sz("24H2")).as_deref(), Some("24H2"));
        assert_eq!(snapshot_string(&sz("")).as_deref(), Some(""));
        assert_eq!(
            snapshot_string(&RegistrySnapshot::Present(RawRegistryValue::dword(0))),
            None
        );
        assert_eq!(snapshot_string(&RegistrySnapshot::ValueMissing), None);
    }

    #[test]
    fn decode_utf16le_stops_at_the_nul() {
        let mut bytes: Vec<u8> = "Professional".encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        bytes.extend_from_slice(&[0, 0]);
        bytes.extend_from_slice(&[0x41, 0x00]); // trailing garbage after the NUL, must be ignored
        assert_eq!(decode_utf16le(&bytes), "Professional");
    }

    #[test]
    fn processor_architecture_maps_to_the_canonical_arch() {
        assert_eq!(map_processor_architecture("AMD64"), "x64");
        assert_eq!(map_processor_architecture("arm64"), "arm64");
        assert_eq!(map_processor_architecture("x86"), "x86");
        // Unknown → lowercased passthrough, which is not a certifiable native arch.
        assert_eq!(map_processor_architecture("IA64"), "ia64");
    }
}
