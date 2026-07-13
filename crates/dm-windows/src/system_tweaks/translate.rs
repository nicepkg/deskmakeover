//! Pure translation + classification core for the calm (清爽) registry backend.
//!
//! Everything in this module compiles on every platform and is unit-tested on the Mac host. The
//! `cfg(windows)` FFI shell in [`super::backend`] performs only the raw Win32 calls and hands the
//! results here as plain integers/bytes, so the branching that actually decides
//! `KeyMissing`/`ValueMissing`/`Present`, conflict detection, policy-management, and post-write
//! verification is verified WITHOUT a Windows box. `[WINDOWS-VERIFY]` therefore covers only the
//! I/O in `backend.rs`, never a decision made here.

use dm_domain::system_tweaks::{
    RawRegistryValue, RegistryAddress, RegistryError, RegistrySnapshot, RegistryValueKind,
    RegistryView,
};

// ── Win32 REGSAM view flags (winnt.h) ────────────────────────────────────────────────────────
// Reproduced as plain constants so the view-selection logic is host-testable without linking
// advapi32. They are stable ABI values; the msvc cross-check would catch any drift against the
// real `windows-rs` constants where the shell composes them.
pub const KEY_WOW64_64KEY: u32 = 0x0100;
pub const KEY_WOW64_32KEY: u32 = 0x0200;

// ── Win32 status codes (winerror.h) the backend branches on ──────────────────────────────────
pub const ERROR_SUCCESS: u32 = 0;
pub const ERROR_FILE_NOT_FOUND: u32 = 2;
pub const ERROR_PATH_NOT_FOUND: u32 = 3;
pub const ERROR_ACCESS_DENIED: u32 = 5;
pub const ERROR_MORE_DATA: u32 = 234;

/// The `KEY_WOW64_*` REGSAM bit for a registry view. `Native` adds none (the process-default
/// view); a recipe pins `Registry64` unless a Windows lab proves a 32-bit-view value, and the view
/// is part of a key's identity so the two never alias.
pub fn view_flag(view: RegistryView) -> u32 {
    match view {
        RegistryView::Native => 0,
        RegistryView::Registry64 => KEY_WOW64_64KEY,
        RegistryView::Registry32 => KEY_WOW64_32KEY,
    }
}

/// The coarse outcome of a `RegOpenKeyExW` / `RegQueryValueExW` — the only distinctions the
/// decision logic needs. Everything that is neither success, not-found, nor access-denied is
/// `Other(code)` and fails closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    NotFound,
    AccessDenied,
    Other(u32),
}

/// Classify a raw Win32 status code. Both `FILE_NOT_FOUND` and `PATH_NOT_FOUND` fold to
/// `NotFound` (a missing value vs a missing key is disambiguated by WHICH call returned it, not by
/// the code).
pub fn classify(status: u32) -> Status {
    match status {
        ERROR_SUCCESS => Status::Ok,
        ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND => Status::NotFound,
        ERROR_ACCESS_DENIED => Status::AccessDenied,
        other => Status::Other(other),
    }
}

/// Assemble a [`RegistrySnapshot`] from a completed key-open + value-query pass:
/// - the key open said `NotFound` → `KeyMissing` (the whole path is absent);
/// - the key opened but the value query said `NotFound` → `ValueMissing`;
/// - both succeeded → `Present` with the EXACT raw type (an extension type `> REG_QWORD` becomes
///   [`RegistryValueKind::Other`], read and preserved byte-for-byte, never normalized);
/// - any access-denied or other status fails closed as [`RegistryError::Io`] so a probe never
///   reads a protected/erroring leaf as a benign absence.
pub fn snapshot_from_query(
    address: &RegistryAddress,
    key_open: Status,
    value_query: Status,
    raw_type: u32,
    bytes: Vec<u8>,
) -> Result<RegistrySnapshot, RegistryError> {
    match key_open {
        Status::NotFound => Ok(RegistrySnapshot::KeyMissing),
        Status::Ok => match value_query {
            Status::NotFound => Ok(RegistrySnapshot::ValueMissing),
            Status::Ok => Ok(RegistrySnapshot::Present(RawRegistryValue::new(
                RegistryValueKind::from_raw(raw_type),
                bytes,
            ))),
            Status::AccessDenied | Status::Other(_) => Err(read_error(address, value_query)),
        },
        Status::AccessDenied | Status::Other(_) => Err(read_error(address, key_open)),
    }
}

/// Whether a leaf is policy-managed, from the status of opening its CONTAINING key for
/// `KEY_SET_VALUE` (a side-effect-free write-access request that writes nothing):
/// - open succeeded → writable → not managed;
/// - `NotFound` → the key is absent, so nothing there is policy-managed (W1 never created it);
/// - `AccessDenied` → the write is refused → treated as policy-managed (do not clobber);
/// - any other status fails closed as an error so the undo/restore caller keeps the transaction
///   prepared rather than proceeding on an unknown state.
///
/// This is the raw-Windows proxy for "can a policy block our write here". It composes with the
/// catalog's declared HKLM `policy_guards`, which the engine reads separately to catch a policy
/// that shadows the value at READ time without denying the write.
pub fn managed_from_open(address: &RegistryAddress, open: Status) -> Result<bool, RegistryError> {
    match open {
        Status::Ok | Status::NotFound => Ok(false),
        Status::AccessDenied => Ok(true),
        Status::Other(code) => Err(RegistryError::Io(format!(
            "policy-managed probe for {address} failed: win32 status {code}"
        ))),
    }
}

/// Whether a key exists, from the status of opening it for `KEY_QUERY_VALUE`. `AccessDenied` fails
/// closed (a key we cannot even open for read is never reported as a clean, writable target).
pub fn exists_from_open(
    key: &dm_domain::system_tweaks::RegistryKey,
    open: Status,
) -> Result<bool, RegistryError> {
    match open {
        Status::Ok => Ok(true),
        Status::NotFound => Ok(false),
        Status::AccessDenied | Status::Other(_) => Err(RegistryError::Io(format!(
            "key-existence probe for {key} failed: {open:?}"
        ))),
    }
}

/// The logical CAS compare: the freshly re-read `actual` must equal the caller's `expected` base,
/// else the write is refused with a [`RegistryError::Conflict`] carrying both sides.
pub fn require_expected(
    address: &RegistryAddress,
    actual: &RegistrySnapshot,
    expected: &RegistrySnapshot,
) -> Result<(), RegistryError> {
    if actual == expected {
        Ok(())
    } else {
        Err(RegistryError::Conflict {
            address: address.clone(),
            expected: Box::new(expected.clone()),
            actual: Box::new(actual.clone()),
        })
    }
}

/// What a CAS must do to establish `desired` at a leaf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteAction<'a> {
    /// Set a concrete standard-kind value (the apply direction, or an undo back to a prior value).
    Set(&'a RawRegistryValue),
    /// Delete just the value, leaving its key (undo of a value W1 created under `CreateAllowed`).
    DeleteValue,
}

/// Resolve the CAS write for `desired`. W1 either sets a concrete standard-kind value or deletes a
/// value it created; it NEVER deletes a KEY via CAS (key creation/teardown is a later slice) and
/// NEVER writes a non-standard `Other` extension type. Both refusals fail closed — defence in depth
/// over the catalog validation, which already rejects an illegal desired at construction.
pub fn plan_write<'a>(
    address: &RegistryAddress,
    desired: &'a RegistrySnapshot,
) -> Result<WriteAction<'a>, RegistryError> {
    match desired {
        RegistrySnapshot::Present(value) if value.kind.is_standard() => Ok(WriteAction::Set(value)),
        RegistrySnapshot::ValueMissing => Ok(WriteAction::DeleteValue),
        RegistrySnapshot::Present(_) => Err(RegistryError::Io(format!(
            "refusing to write a non-standard extension-type value at {address}"
        ))),
        RegistrySnapshot::KeyMissing => Err(RegistryError::Io(format!(
            "refusing to delete a key via CAS at {address} (W1 creates no keys)"
        ))),
    }
}

/// Post-write verification: the value re-read after the set must byte-match the desired write. A
/// mismatch (a torn write, a virtualized redirect, an instant external overwrite) fails closed so
/// the transaction never records a write that did not land as intended.
pub fn verify_readback(
    address: &RegistryAddress,
    readback: &RegistrySnapshot,
    desired: &RegistrySnapshot,
) -> Result<(), RegistryError> {
    if readback == desired {
        Ok(())
    } else {
        Err(RegistryError::Io(format!(
            "post-write readback at {address} did not match the value written"
        )))
    }
}

/// A read failure, coarsely: an access-denied or any other non-`NotFound` status. `NotFound` is
/// never routed here (it is a legitimate `KeyMissing`/`ValueMissing`, not an error).
fn read_error(address: &RegistryAddress, status: Status) -> RegistryError {
    RegistryError::Io(format!("registry read at {address} failed: {status:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dm_domain::system_tweaks::{RegistryHive, RegistryKey};

    fn addr() -> RegistryAddress {
        RegistryAddress::new(
            RegistryHive::CurrentUser,
            RegistryView::Registry64,
            r"Software\Test",
            "Value",
        )
    }

    #[test]
    fn view_flag_pins_the_wow64_bit_per_view() {
        assert_eq!(view_flag(RegistryView::Native), 0);
        assert_eq!(view_flag(RegistryView::Registry64), KEY_WOW64_64KEY);
        assert_eq!(view_flag(RegistryView::Registry32), KEY_WOW64_32KEY);
        // The two explicit views never collapse to the same REGSAM bit.
        assert_ne!(
            view_flag(RegistryView::Registry64),
            view_flag(RegistryView::Registry32)
        );
    }

    #[test]
    fn classify_maps_the_codes_the_backend_branches_on() {
        assert_eq!(classify(ERROR_SUCCESS), Status::Ok);
        assert_eq!(classify(ERROR_FILE_NOT_FOUND), Status::NotFound);
        assert_eq!(classify(ERROR_PATH_NOT_FOUND), Status::NotFound);
        assert_eq!(classify(ERROR_ACCESS_DENIED), Status::AccessDenied);
        assert_eq!(classify(1234), Status::Other(1234));
    }

    #[test]
    fn a_missing_key_is_key_missing_even_when_the_value_query_is_bogus() {
        // The value query is never made when the key open failed; a stray status must not matter.
        let snap = snapshot_from_query(&addr(), Status::NotFound, Status::Ok, 4, vec![1, 0, 0, 0]);
        assert_eq!(snap, Ok(RegistrySnapshot::KeyMissing));
    }

    #[test]
    fn a_present_key_missing_value_is_value_missing() {
        let snap = snapshot_from_query(&addr(), Status::Ok, Status::NotFound, 0, Vec::new());
        assert_eq!(snap, Ok(RegistrySnapshot::ValueMissing));
    }

    #[test]
    fn a_present_dword_reads_back_its_exact_bytes_and_kind() {
        let snap =
            snapshot_from_query(&addr(), Status::Ok, Status::Ok, 4, vec![1, 0, 0, 0]).unwrap();
        assert_eq!(
            snap,
            RegistrySnapshot::Present(RawRegistryValue::dword(1))
        );
    }

    #[test]
    fn an_extension_type_is_preserved_as_other_never_normalized() {
        // A raw type > REG_QWORD (11) must survive as Other(raw) with its exact bytes so a restore
        // is byte-identical and `accepts` can fail it closed — the winreg typed layer cannot do
        // this (it errors on such a value), which is why the backend uses raw FFI.
        let bytes = vec![0xde, 0xad, 0xbe, 0xef];
        let snap =
            snapshot_from_query(&addr(), Status::Ok, Status::Ok, 42, bytes.clone()).unwrap();
        match snap {
            RegistrySnapshot::Present(value) => {
                assert_eq!(value.kind, RegistryValueKind::Other(42));
                assert!(!value.kind.is_standard());
                assert_eq!(value.bytes, bytes);
            }
            other => panic!("expected Present(Other), got {other:?}"),
        }
    }

    #[test]
    fn a_read_that_is_denied_or_errors_fails_closed_never_absent() {
        assert!(matches!(
            snapshot_from_query(&addr(), Status::AccessDenied, Status::Ok, 0, Vec::new()),
            Err(RegistryError::Io(_))
        ));
        assert!(matches!(
            snapshot_from_query(&addr(), Status::Ok, Status::Other(87), 0, Vec::new()),
            Err(RegistryError::Io(_))
        ));
    }

    #[test]
    fn managed_probe_reads_write_denial_as_management() {
        assert_eq!(managed_from_open(&addr(), Status::Ok), Ok(false));
        assert_eq!(managed_from_open(&addr(), Status::NotFound), Ok(false));
        assert_eq!(managed_from_open(&addr(), Status::AccessDenied), Ok(true));
        assert!(matches!(
            managed_from_open(&addr(), Status::Other(1)),
            Err(RegistryError::Io(_))
        ));
    }

    #[test]
    fn key_existence_fails_closed_on_denial() {
        let key = RegistryKey::new(
            RegistryHive::CurrentUser,
            RegistryView::Registry64,
            r"Software\Test",
        );
        assert_eq!(exists_from_open(&key, Status::Ok), Ok(true));
        assert_eq!(exists_from_open(&key, Status::NotFound), Ok(false));
        assert!(matches!(
            exists_from_open(&key, Status::AccessDenied),
            Err(RegistryError::Io(_))
        ));
    }

    #[test]
    fn require_expected_rejects_a_moved_base_with_both_sides() {
        let expected = RegistrySnapshot::Present(RawRegistryValue::dword(1));
        let actual = RegistrySnapshot::Present(RawRegistryValue::dword(0));
        assert_eq!(require_expected(&addr(), &expected, &expected), Ok(()));
        match require_expected(&addr(), &actual, &expected) {
            Err(RegistryError::Conflict {
                expected: e,
                actual: a,
                ..
            }) => {
                assert_eq!(*e, expected);
                assert_eq!(*a, actual);
            }
            other => panic!("expected Conflict, got {other:?}"),
        }
    }

    #[test]
    fn plan_write_sets_a_value_deletes_a_created_value_and_refuses_the_rest() {
        let dword = RegistrySnapshot::Present(RawRegistryValue::dword(0));
        match plan_write(&addr(), &dword) {
            Ok(WriteAction::Set(value)) => assert_eq!(value.as_dword(), Some(0)),
            other => panic!("expected Set, got {other:?}"),
        }
        // Undo of a W1-created value deletes just the value (key stays).
        assert_eq!(
            plan_write(&addr(), &RegistrySnapshot::ValueMissing),
            Ok(WriteAction::DeleteValue)
        );
        // A key deletion via CAS is refused (W1 creates no keys).
        assert!(plan_write(&addr(), &RegistrySnapshot::KeyMissing).is_err());
        // A non-standard extension type is never written.
        let other = RegistrySnapshot::Present(RawRegistryValue::new(
            RegistryValueKind::Other(42),
            vec![0, 0, 0, 0],
        ));
        assert!(plan_write(&addr(), &other).is_err());
    }

    #[test]
    fn verify_readback_requires_byte_equality_with_the_write() {
        let desired = RegistrySnapshot::Present(RawRegistryValue::dword(0));
        assert_eq!(verify_readback(&addr(), &desired, &desired), Ok(()));
        let torn = RegistrySnapshot::Present(RawRegistryValue::dword(1));
        assert!(matches!(
            verify_readback(&addr(), &torn, &desired),
            Err(RegistryError::Io(_))
        ));
    }
}
