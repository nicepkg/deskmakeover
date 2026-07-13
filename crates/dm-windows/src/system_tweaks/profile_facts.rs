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
// host-tested here rather than behind `[WINDOWS-VERIFY]`. Every extractor distinguishes ABSENT
// (`Ok(None)` — the caller decides whether that is fatal or a legitimate default) from PRESENT BUT
// MALFORMED (`Err` — never silently coerced), so hostile or corrupt registry data can never be
// folded into a benign default and then match a certified profile (codex W2 R1 Major-4/5).

/// A present-but-malformed certification field: the value existed but could not be read as the
/// expected type/shape. Kept distinct from an absent field so the probe fails closed on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MalformedField(pub String);

/// The `u32` a present DWORD snapshot holds:
/// - key/value absent → `Ok(None)` (the caller may default, e.g. a missing UBR is 0 on old builds);
/// - present, a well-formed 4-byte DWORD → `Ok(Some(value))`;
/// - present but a non-DWORD kind or a malformed width → `Err` (never a silent `0`).
pub fn snapshot_dword(snapshot: &RegistrySnapshot) -> Result<Option<u32>, MalformedField> {
    match snapshot.value() {
        None => Ok(None),
        Some(value) => value.as_dword().map(Some).ok_or_else(|| {
            MalformedField(format!(
                "expected a 4-byte DWORD, found kind {:?} of {} bytes",
                value.kind,
                value.bytes.len()
            ))
        }),
    }
}

/// The string a present `REG_SZ`/`REG_EXPAND_SZ` snapshot holds:
/// - key/value absent → `Ok(None)`;
/// - present, a valid UTF-16LE string → `Ok(Some(text))`;
/// - present but a non-string kind, an odd byte length, or invalid UTF-16 → `Err`.
pub fn snapshot_string(snapshot: &RegistrySnapshot) -> Result<Option<String>, MalformedField> {
    use dm_domain::system_tweaks::RegistryValueKind::{ExpandString, String as Sz};
    match snapshot.value() {
        None => Ok(None),
        Some(value) => match value.kind {
            Sz | ExpandString => decode_utf16le(&value.bytes).map(Some),
            ref other => Err(MalformedField(format!(
                "expected a string value, found kind {other:?}"
            ))),
        },
    }
}

/// Decode a registry UTF-16LE string blob, stopping at the first NUL terminator. Fails closed on an
/// ODD byte length (a well-formed registry string is always an even byte count) and on INVALID
/// UTF-16 (unpaired surrogates) — `String::from_utf16` (not `_lossy`) is used, because a
/// certification identity must never accept a mangled string as a valid field.
pub fn decode_utf16le(bytes: &[u8]) -> Result<String, MalformedField> {
    if !bytes.len().is_multiple_of(2) {
        return Err(MalformedField(format!(
            "registry string has an odd byte length ({})",
            bytes.len()
        )));
    }
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    let end = units.iter().position(|&unit| unit == 0).unwrap_or(units.len());
    String::from_utf16(&units[..end])
        .map_err(|error| MalformedField(format!("registry string is not valid UTF-16: {error}")))
}

/// Map a raw `PROCESSOR_ARCHITECTURE` value (from `GetNativeSystemInfo`) to the canonical
/// architecture the domain certifies. An unrecognized value becomes a distinct non-certifiable
/// string rather than masquerading as a supported arch.
pub fn map_native_arch(raw: u16) -> String {
    match raw {
        9 => "x64".to_string(),    // PROCESSOR_ARCHITECTURE_AMD64
        12 => "arm64".to_string(), // PROCESSOR_ARCHITECTURE_ARM64
        0 => "x86".to_string(),    // PROCESSOR_ARCHITECTURE_INTEL
        other => format!("arch{other}"),
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
    fn snapshot_dword_absent_is_none_present_is_some_malformed_is_err() {
        assert_eq!(
            snapshot_dword(&RegistrySnapshot::Present(RawRegistryValue::dword(8_737))),
            Ok(Some(8_737))
        );
        assert_eq!(snapshot_dword(&RegistrySnapshot::ValueMissing), Ok(None));
        assert_eq!(snapshot_dword(&RegistrySnapshot::KeyMissing), Ok(None));
        // A string value at a DWORD field is malformed, NOT a silent absence (codex W2 R1 Major-4).
        assert!(snapshot_dword(&sz("26100")).is_err());
        // A present but wrong-width DWORD is malformed, never a silent 0.
        let narrow = RegistrySnapshot::Present(RawRegistryValue::new(
            RegistryValueKind::Dword,
            vec![0, 0],
        ));
        assert!(snapshot_dword(&narrow).is_err());
    }

    #[test]
    fn snapshot_string_absent_is_none_valid_is_some_malformed_is_err() {
        assert_eq!(snapshot_string(&sz("24H2")), Ok(Some("24H2".to_string())));
        assert_eq!(snapshot_string(&sz("")), Ok(Some(String::new())));
        assert_eq!(snapshot_string(&RegistrySnapshot::ValueMissing), Ok(None));
        // A DWORD at a string field is malformed, not a silent absence.
        assert!(snapshot_string(&RegistrySnapshot::Present(RawRegistryValue::dword(0))).is_err());
    }

    #[test]
    fn decode_utf16le_stops_at_the_nul_but_rejects_malformed() {
        let mut bytes: Vec<u8> = "Professional".encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        bytes.extend_from_slice(&[0, 0]);
        bytes.extend_from_slice(&[0x41, 0x00]); // trailing garbage after the NUL, must be ignored
        assert_eq!(decode_utf16le(&bytes).unwrap(), "Professional");
        // An odd trailing byte is a malformed field, NOT silently dropped (codex W2 R1 Major-5).
        let mut odd: Vec<u8> = "Professional".encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        odd.push(0x41);
        assert!(decode_utf16le(&odd).is_err());
        // An unpaired high surrogate is invalid UTF-16 → Err (from_utf16, not _lossy).
        let bad_surrogate: Vec<u8> = vec![0x00, 0xD8]; // U+D800 alone
        assert!(decode_utf16le(&bad_surrogate).is_err());
    }

    #[test]
    fn native_arch_maps_to_the_canonical_arch() {
        assert_eq!(map_native_arch(9), "x64"); // PROCESSOR_ARCHITECTURE_AMD64
        assert_eq!(map_native_arch(12), "arm64"); // PROCESSOR_ARCHITECTURE_ARM64
        assert_eq!(map_native_arch(0), "x86"); // PROCESSOR_ARCHITECTURE_INTEL
        // Unknown → a distinct non-certifiable string.
        assert_eq!(map_native_arch(6), "arch6"); // IA64, not certifiable
    }
}
