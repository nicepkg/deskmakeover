//! `WinregBackend` — the Windows registry adapter for the calm (清爽) decision core. `[WINDOWS-VERIFY]`.
//!
//! A thin `cfg(windows)` shell over raw `windows-rs` registry FFI. Every DECISION
//! (`KeyMissing`/`ValueMissing`/`Present`, CAS conflict, policy-management, post-write
//! verification) is made by the host-tested pure functions in [`super::translate`]; this file only
//! performs the syscalls and hands their status codes + bytes to that logic, so a Windows box is
//! needed only to confirm the I/O, never the branching.
//!
//! It deliberately uses raw `RegQueryValueExW`/`RegSetValueExW` rather than the typed `winreg`
//! layer: `winreg::get_raw_value` returns `ERROR_BAD_FILE_TYPE` for any value type above
//! `REG_QWORD`, so it cannot read/preserve the domain's `RegistryValueKind::Other` extension types
//! byte-for-byte — which `SettingMutation::accepts` must be able to see in order to fail closed.

use dm_domain::system_tweaks::{
    DeleteKeyOutcome, RegistryAddress, RegistryBackend, RegistryError, RegistryHive, RegistryKey,
    RegistrySnapshot, RegistryView, RegistryWriteIntent, RegistryWriteOutcome,
};
use windows::core::PCWSTR;
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SAM_FLAGS,
    REG_VALUE_TYPE,
};

use super::translate::{self, Status, WriteAction};

/// The Windows registry backend. Stateless: each call opens exactly the handles it needs and closes
/// them (via [`OwnedHkey`]) before returning, so there is no cross-call handle state to leak. The
/// driver's writer lease plus the host mutex serialize writers; this type adds no locking of its own.
#[derive(Debug, Default)]
pub struct WinregBackend;

impl WinregBackend {
    pub fn new() -> Self {
        Self
    }
}

/// An owned open registry handle, closed exactly once on drop so every early-return path is
/// leak-free.
struct OwnedHkey(HKEY);

impl Drop for OwnedHkey {
    fn drop(&mut self) {
        // SAFETY: `self.0` was produced by a successful `RegOpenKeyExW` and is closed exactly once
        // (this `Drop`); the handle is never copied out to be closed elsewhere.
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

/// A NUL-terminated UTF-16 buffer for a `PCWSTR` argument. The returned `Vec` must outlive the call.
fn to_wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn root_hkey(hive: RegistryHive) -> HKEY {
    match hive {
        RegistryHive::CurrentUser => HKEY_CURRENT_USER,
        RegistryHive::LocalMachine => HKEY_LOCAL_MACHINE,
    }
}

/// The REGSAM for `base` in `view` (the pure `KEY_WOW64_*` selection lives in `translate`).
fn sam(base: REG_SAM_FLAGS, view: RegistryView) -> REG_SAM_FLAGS {
    REG_SAM_FLAGS(base.0 | translate::view_flag(view))
}

/// Open a key with the requested access, returning the coarse [`Status`] and the handle on success.
fn open_key(key: &RegistryKey, base: REG_SAM_FLAGS) -> (Status, Option<OwnedHkey>) {
    let wide = to_wide(&key.path);
    let mut handle = HKEY::default();
    // SAFETY: a valid predefined root, a NUL-terminated subkey held by `wide` for the call, and an
    // out-param handle. On success the handle is owned by `OwnedHkey` and closed on drop.
    let status = unsafe {
        RegOpenKeyExW(
            root_hkey(key.hive),
            PCWSTR(wide.as_ptr()),
            None,
            sam(base, key.view),
            &mut handle,
        )
    };
    match translate::classify(status.0) {
        Status::Ok => (Status::Ok, Some(OwnedHkey(handle))),
        other => (other, None),
    }
}

/// Read a value under an already-open key: `(status, raw_type, bytes)`. Two-pass — size/type first
/// (`lpData` null), then the exact bytes — so a value of any length is captured without truncation.
fn query_value(handle: &OwnedHkey, value_name: &str) -> (Status, u32, Vec<u8>) {
    let wide = to_wide(value_name);
    let name = PCWSTR(wide.as_ptr());
    let mut ty = REG_VALUE_TYPE(0);
    let mut cb: u32 = 0;
    // SAFETY: open handle, NUL-terminated name held by `wide`, out-params for type + size; `lpData`
    // is null on this sizing pass so nothing is written to a buffer.
    let sized = unsafe {
        RegQueryValueExW(handle.0, name, None, Some(&mut ty), None, Some(&mut cb))
    };
    match translate::classify(sized.0) {
        Status::Ok => {}
        other => return (other, ty.0, Vec::new()),
    }
    if cb == 0 {
        return (Status::Ok, ty.0, Vec::new());
    }
    let mut buf = vec![0u8; cb as usize];
    // SAFETY: `buf` holds `cb` bytes and `cb` is passed as the buffer capacity, so the write stays
    // in bounds; the type is re-read to stay consistent with the sizing pass.
    let read = unsafe {
        RegQueryValueExW(
            handle.0,
            name,
            None,
            Some(&mut ty),
            Some(buf.as_mut_ptr()),
            Some(&mut cb),
        )
    };
    match translate::classify(read.0) {
        Status::Ok => {
            buf.truncate(cb as usize);
            (Status::Ok, ty.0, buf)
        }
        other => (other, ty.0, Vec::new()),
    }
}

impl RegistryBackend for WinregBackend {
    fn read(&self, address: &RegistryAddress) -> Result<RegistrySnapshot, RegistryError> {
        let (key_open, handle) = open_key(&address.key_location(), KEY_QUERY_VALUE);
        let (value_query, raw_type, bytes) = match &handle {
            Some(open) => query_value(open, &address.value),
            None => (Status::NotFound, 0, Vec::new()), // ignored unless key_open == Ok
        };
        translate::snapshot_from_query(address, key_open, value_query, raw_type, bytes)
    }

    fn key_exists(&self, key: &RegistryKey) -> Result<bool, RegistryError> {
        if key.is_hive_root() {
            return Ok(true);
        }
        let (status, _handle) = open_key(key, KEY_QUERY_VALUE);
        translate::exists_from_open(key, status)
    }

    fn is_policy_managed(&self, address: &RegistryAddress) -> Result<bool, RegistryError> {
        // Request write access to the CONTAINING key; a denial means a policy protects the leaf.
        // This writes nothing — it only asks for the right — so it is safe in a read-only classify.
        let (status, _handle) = open_key(&address.key_location(), KEY_SET_VALUE);
        translate::managed_from_open(address, status)
    }

    fn compare_exchange(
        &mut self,
        intent: RegistryWriteIntent,
        address: &RegistryAddress,
        expected: &RegistrySnapshot,
        desired: &RegistrySnapshot,
    ) -> Result<RegistryWriteOutcome, RegistryError> {
        // Apply and Undo are gated identically here; a policy that took the leaf is discovered at
        // write time as an access denial and surfaced as `ManagedByPolicy` for either direction.
        let _ = intent;

        // Logical CAS: re-read and require the expected base before writing.
        let actual = self.read(address)?;
        translate::require_expected(address, &actual, expected)?;

        let action = translate::plan_write(address, desired)?;
        let (open_status, handle) = open_key(&address.key_location(), KEY_SET_VALUE);
        let handle = match open_status {
            Status::Ok => handle.expect("an Ok open yields a handle"),
            Status::AccessDenied => return Err(RegistryError::ManagedByPolicy(address.clone())),
            Status::NotFound => {
                return Err(RegistryError::Io(format!(
                    "key vanished before write at {address}"
                )))
            }
            Status::Other(code) => {
                return Err(RegistryError::Io(format!(
                    "open-for-write at {address} failed: win32 status {code}"
                )))
            }
        };

        let name_wide = to_wide(&address.value);
        let name = PCWSTR(name_wide.as_ptr());
        let write_status = match action {
            // SAFETY: open write handle, NUL-terminated name held by `name_wide`, and the exact
            // desired bytes with their exact Win32 type number.
            WriteAction::Set(value) => unsafe {
                RegSetValueExW(
                    handle.0,
                    name,
                    None,
                    REG_VALUE_TYPE(value.kind.raw()),
                    Some(&value.bytes),
                )
            },
            // SAFETY: open write handle and NUL-terminated name held by `name_wide`.
            WriteAction::DeleteValue => unsafe { RegDeleteValueW(handle.0, name) },
        };
        match translate::classify(write_status.0) {
            Status::Ok => {}
            // Deleting an already-absent value is idempotent to the ValueMissing we intended.
            Status::NotFound if matches!(desired, RegistrySnapshot::ValueMissing) => {}
            Status::AccessDenied => return Err(RegistryError::ManagedByPolicy(address.clone())),
            other => {
                return Err(RegistryError::Io(format!(
                    "write at {address} failed: {other:?}"
                )))
            }
        }
        drop(handle); // close the write handle before the verifying re-read

        // Post-write verification: the value must now read back exactly as written.
        let readback = self.read(address)?;
        translate::verify_readback(address, &readback, desired)?;

        // W1 opens keys, never creates them, so there are no created prefixes to record.
        Ok(RegistryWriteOutcome::default())
    }

    fn delete_key_if_empty(
        &mut self,
        _key: &RegistryKey,
    ) -> Result<DeleteKeyOutcome, RegistryError> {
        // W1 recipes create no key (writes open pre-existing keys; a Present write into a missing
        // key fails the open rather than materializing it), so `compare_exchange` never records a
        // created prefix and cleanup is never handed one. Reverse-order key teardown lands with the
        // key-creation slice; until then this is provably unreachable and returns the empty outcome.
        Ok(DeleteKeyOutcome::AlreadyMissing)
    }
}
