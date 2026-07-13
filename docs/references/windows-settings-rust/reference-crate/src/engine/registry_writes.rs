use crate::{
    JournalStore, RegistryAddress, RegistryBackend, RegistryError, RegistrySnapshot,
    RegistryWriteIntent, RegistryWriteOutcome, RuntimeProbe, TransactionValue, VerificationBackend,
};

use super::registry_keys::leaf_restore_target;
use super::SettingsEngine;

impl<B, J, V, R> SettingsEngine<B, J, V, R>
where
    B: RegistryBackend,
    J: JournalStore,
    V: VerificationBackend<B>,
    R: RuntimeProbe,
{
    pub(super) fn begin_write(
        &mut self,
        intent: RegistryWriteIntent,
        address: &RegistryAddress,
        expected: &RegistrySnapshot,
        desired: &RegistrySnapshot,
    ) -> Result<Option<(RegistryWriteOutcome, RegistrySnapshot)>, RegistryError> {
        if expected == desired {
            self.verify_immediate_readback(address, desired)?;
            return Ok(None);
        }
        let leaf_target = leaf_restore_target(desired);
        let outcome = self
            .backend
            .compare_exchange(intent, address, expected, &leaf_target)?;
        Ok(Some((outcome, leaf_target)))
    }

    pub(super) fn write_or_verify(
        &mut self,
        intent: RegistryWriteIntent,
        address: &RegistryAddress,
        expected: &RegistrySnapshot,
        desired: &RegistrySnapshot,
    ) -> Result<(), RegistryError> {
        if let Some((_, leaf_target)) = self.begin_write(intent, address, expected, desired)? {
            self.verify_immediate_readback(address, &leaf_target)?;
        }
        Ok(())
    }

    pub(super) fn verify_immediate_readback(
        &self,
        address: &RegistryAddress,
        desired: &RegistrySnapshot,
    ) -> Result<(), RegistryError> {
        let first = self.backend.read(address)?;
        let second = self.backend.read(address)?;
        if &first != desired || &second != desired || first != second {
            return Err(RegistryError::VerificationFailed {
                address: address.clone(),
                desired: Box::new(desired.clone()),
                first: Box::new(first),
                second: Box::new(second),
            });
        }
        Ok(())
    }

    pub(super) fn rollback_to_original(
        &mut self,
        values: &[TransactionValue],
    ) -> Result<(), RegistryError> {
        let mut first_error = None;
        for value in values.iter().rev() {
            let leaf_original = leaf_restore_target(&value.original);
            let result = match self.backend.read(&value.address) {
                Ok(current) if current == value.original => {
                    self.verify_immediate_readback(&value.address, &value.original)
                }
                Ok(current) if current == leaf_original => {
                    self.verify_immediate_readback(&value.address, &leaf_original)
                }
                Ok(current) if current == value.before || current == value.desired => self
                    .write_or_verify(
                        RegistryWriteIntent::Undo,
                        &value.address,
                        &current,
                        &value.original,
                    ),
                Ok(current) => Err(RegistryError::Conflict {
                    address: value.address.clone(),
                    expected: Box::new(value.desired.clone()),
                    actual: Box::new(current),
                }),
                Err(error) => Err(error),
            };
            if first_error.is_none() {
                first_error = result.err();
            }
        }
        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}
