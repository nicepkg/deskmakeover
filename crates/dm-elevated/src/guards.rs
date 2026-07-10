//! Pure LPE guards for the overlay ICO. Ported from `OverlayCommands.IsIco` + the size cap.
//! The registry NEVER points at a caller-supplied path (ADR-0021 §4): the helper validates the
//! ICO and copies it into `%ProgramData%` first. These checks are the validation half.

/// The custom overlay ICO size cap (oracle: `MaxCustomIcoBytes`).
pub const MAX_ICO_BYTES: u64 = 5 * 1024 * 1024;

/// Checks the ICONDIR magic: reserved = 0, type = 1 (icon), count ≥ 1 (oracle `IsIco`).
pub fn is_ico_header(header: &[u8]) -> bool {
    header.len() >= 6
        && header[0] == 0
        && header[1] == 0
        && header[2] == 1
        && header[3] == 0
        && (header[4] > 0 || header[5] > 0)
}

/// Validates an overlay ICO before it is trusted: non-empty, within the size cap, and a real
/// icon container. Applied to every style (built-in and custom) so the registry only ever points
/// at a validated, DeskMakeover-owned `%ProgramData%` copy.
pub fn validate_ico(size_bytes: u64, header: &[u8]) -> Result<(), String> {
    if size_bytes == 0 {
        return Err("overlay ico is empty".to_string());
    }
    if size_bytes > MAX_ICO_BYTES {
        return Err(format!("overlay ico size {size_bytes} exceeds the {MAX_ICO_BYTES}-byte cap"));
    }
    if !is_ico_header(header) {
        return Err("overlay file is not a valid .ico (bad ICONDIR)".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ico_header() -> [u8; 6] {
        [0, 0, 1, 0, 1, 0] // reserved=0, type=1, count=1
    }

    #[test]
    fn accepts_a_valid_icondir() {
        assert!(is_ico_header(&ico_header()));
        assert!(validate_ico(1024, &ico_header()).is_ok());
    }

    #[test]
    fn rejects_non_ico_headers() {
        assert!(!is_ico_header(&[0x89, b'P', b'N', b'G', 0, 0])); // PNG
        assert!(!is_ico_header(&[0, 0, 2, 0, 1, 0])); // type=2 (cursor), not icon
        assert!(!is_ico_header(&[0, 0, 1, 0, 0, 0])); // count=0
        assert!(!is_ico_header(&[0, 0])); // too short
        assert!(validate_ico(1024, &[0x89, b'P', b'N', b'G']).is_err());
    }

    #[test]
    fn rejects_empty_and_oversized() {
        assert!(validate_ico(0, &ico_header()).is_err());
        assert!(validate_ico(MAX_ICO_BYTES + 1, &ico_header()).is_err());
        assert!(validate_ico(MAX_ICO_BYTES, &ico_header()).is_ok());
    }
}
