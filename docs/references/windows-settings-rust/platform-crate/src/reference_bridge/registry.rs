use deskmakeover_windows_settings_reference as reference;

use crate::{
    DeleteKeyOutcome as PlatformDeleteKeyOutcome, KeyDisposition,
    RawRegistryValue as PlatformRawValue, RegistryBackend as PlatformRegistryBackend,
    RegistryError as PlatformRegistryError, RegistryHive as PlatformHive, RegistryLocation,
    RegistrySnapshot as PlatformSnapshot, RegistryView as PlatformView,
};

/// Per-setting GPO/MDM precedence is product knowledge, not a registry default. Callers must inject
/// an implementation; this bridge intentionally provides no always-false fallback.
pub trait PolicyStateProbe {
    fn is_policy_managed(&self, location: &RegistryLocation)
        -> Result<bool, PlatformRegistryError>;
}

#[derive(Debug)]
pub struct ReferenceRegistryBackend<R, P> {
    registry: R,
    policy: P,
}

impl<R, P> ReferenceRegistryBackend<R, P> {
    pub fn new(registry: R, policy: P) -> Self {
        Self { registry, policy }
    }
}

impl<R, P> reference::RegistryBackend for ReferenceRegistryBackend<R, P>
where
    R: PlatformRegistryBackend,
    P: PolicyStateProbe,
{
    fn read(
        &self,
        address: &reference::RegistryAddress,
    ) -> Result<reference::RegistrySnapshot, reference::RegistryError> {
        let location = address_location(address);
        self.registry
            .read_value(&location)
            .map(snapshot_to_reference)
            .map_err(registry_io)
    }

    fn key_exists(&self, key: &reference::RegistryKey) -> Result<bool, reference::RegistryError> {
        self.registry
            .key_exists(&key_location(key))
            .map_err(registry_io)
    }

    fn is_policy_managed(
        &self,
        address: &reference::RegistryAddress,
    ) -> Result<bool, reference::RegistryError> {
        self.policy
            .is_policy_managed(&address_location(address))
            .map_err(registry_io)
    }

    fn compare_exchange(
        &mut self,
        intent: reference::RegistryWriteIntent,
        address: &reference::RegistryAddress,
        expected: &reference::RegistrySnapshot,
        desired: &reference::RegistrySnapshot,
    ) -> Result<reference::RegistryWriteOutcome, reference::RegistryError> {
        let location = address_location(address);
        if intent == reference::RegistryWriteIntent::Apply
            && self
                .policy
                .is_policy_managed(&location)
                .map_err(registry_io)?
        {
            return Err(reference::RegistryError::ManagedByPolicy(address.clone()));
        }
        let actual = self
            .registry
            .read_value(&location)
            .map(snapshot_to_reference)
            .map_err(registry_io)?;
        if &actual != expected {
            return Err(reference::RegistryError::Conflict {
                address: address.clone(),
                expected: Box::new(expected.clone()),
                actual: Box::new(actual),
            });
        }

        let mut created_keys = Vec::new();
        match desired {
            reference::RegistrySnapshot::Present(value) => {
                let value = raw_to_platform(value)?;
                if matches!(actual, reference::RegistrySnapshot::KeyMissing) {
                    for prefix in address.key_location().prefixes() {
                        if self
                            .registry
                            .create_key(&key_location(&prefix))
                            .map_err(registry_io)?
                            == KeyDisposition::Created
                        {
                            created_keys.push(prefix);
                        }
                    }
                }
                self.registry
                    .write_value(&location, &value)
                    .map_err(registry_io)?;
            }
            reference::RegistrySnapshot::ValueMissing => {
                if matches!(actual, reference::RegistrySnapshot::KeyMissing) {
                    return Err(reference::RegistryError::Io(
                        "cannot materialize an empty registry key through value CAS".into(),
                    ));
                }
                self.registry.delete_value(&location).map_err(registry_io)?;
            }
            reference::RegistrySnapshot::KeyMissing => {
                // Created-key cleanup is a separate, journal-driven deepest-first operation.
                self.registry.delete_value(&location).map_err(registry_io)?;
            }
        }
        Ok(reference::RegistryWriteOutcome { created_keys })
    }

    fn delete_key_if_empty(
        &mut self,
        key: &reference::RegistryKey,
    ) -> Result<reference::DeleteKeyOutcome, reference::RegistryError> {
        self.registry
            .delete_key_if_empty(&key_location(key))
            .map(|outcome| match outcome {
                PlatformDeleteKeyOutcome::Deleted => reference::DeleteKeyOutcome::Deleted,
                PlatformDeleteKeyOutcome::AlreadyMissing => {
                    reference::DeleteKeyOutcome::AlreadyMissing
                }
                PlatformDeleteKeyOutcome::NotEmpty => reference::DeleteKeyOutcome::NotEmpty,
            })
            .map_err(registry_io)
    }
}

fn address_location(address: &reference::RegistryAddress) -> RegistryLocation {
    RegistryLocation::new(
        hive_to_platform(address.hive),
        address.key.clone(),
        address.value.clone(),
        view_to_platform(address.view),
    )
}

fn key_location(key: &reference::RegistryKey) -> RegistryLocation {
    RegistryLocation::new(
        hive_to_platform(key.hive),
        key.path.clone(),
        "",
        view_to_platform(key.view),
    )
}

fn hive_to_platform(hive: reference::RegistryHive) -> PlatformHive {
    match hive {
        reference::RegistryHive::CurrentUser => PlatformHive::CurrentUser,
        reference::RegistryHive::LocalMachine => PlatformHive::LocalMachine,
    }
}

fn view_to_platform(view: reference::RegistryView) -> PlatformView {
    match view {
        reference::RegistryView::Native => PlatformView::Native,
        reference::RegistryView::Registry32 => PlatformView::View32,
        reference::RegistryView::Registry64 => PlatformView::View64,
    }
}

fn snapshot_to_reference(snapshot: PlatformSnapshot) -> reference::RegistrySnapshot {
    match (snapshot.key_existed, snapshot.value) {
        (false, _) => reference::RegistrySnapshot::KeyMissing,
        (true, None) => reference::RegistrySnapshot::ValueMissing,
        (true, Some(value)) => reference::RegistrySnapshot::Present(reference::RawRegistryValue {
            kind: kind_to_reference(value.kind),
            bytes: value.bytes,
        }),
    }
}

fn kind_to_reference(kind: u32) -> reference::RegistryValueKind {
    match kind {
        0 => reference::RegistryValueKind::None,
        1 => reference::RegistryValueKind::String,
        2 => reference::RegistryValueKind::ExpandString,
        3 => reference::RegistryValueKind::Binary,
        4 => reference::RegistryValueKind::Dword,
        7 => reference::RegistryValueKind::MultiString,
        11 => reference::RegistryValueKind::Qword,
        value => reference::RegistryValueKind::Other(value),
    }
}

fn raw_to_platform(
    value: &reference::RawRegistryValue,
) -> Result<PlatformRawValue, reference::RegistryError> {
    let kind = match value.kind {
        reference::RegistryValueKind::None => 0,
        reference::RegistryValueKind::String => 1,
        reference::RegistryValueKind::ExpandString => 2,
        reference::RegistryValueKind::Binary => 3,
        reference::RegistryValueKind::Dword => 4,
        reference::RegistryValueKind::MultiString => 7,
        reference::RegistryValueKind::Qword => 11,
        reference::RegistryValueKind::Other(kind @ (5 | 6 | 8 | 9 | 10)) => kind,
        reference::RegistryValueKind::Other(kind) => {
            return Err(reference::RegistryError::Io(format!(
                "unsupported raw REG_* kind {kind}; refusing lossy restore"
            )));
        }
    };
    Ok(PlatformRawValue {
        kind,
        bytes: value.bytes.clone(),
    })
}

fn registry_io(error: PlatformRegistryError) -> reference::RegistryError {
    reference::RegistryError::Io(error.to_string())
}
