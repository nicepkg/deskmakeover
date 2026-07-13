use deskmakeover_windows_settings_reference::{
    LockScreenBackground, RuntimeFacts, RuntimeProbe, RuntimeProbeError, WindowsEnvironment,
};

use crate::SystemProfileProbe;

pub trait LockScreenBackgroundProbe {
    fn probe(&self) -> Result<LockScreenBackground, RuntimeProbeError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct UnknownLockScreenBackgroundProbe;

impl LockScreenBackgroundProbe for UnknownLockScreenBackgroundProbe {
    fn probe(&self) -> Result<LockScreenBackground, RuntimeProbeError> {
        Ok(LockScreenBackground::Unknown)
    }
}

#[derive(Debug)]
pub struct ReferenceRuntimeProbe<S, L> {
    system_profile: S,
    lock_screen_background: L,
}

impl<S, L> ReferenceRuntimeProbe<S, L> {
    pub fn new(system_profile: S, lock_screen_background: L) -> Self {
        Self {
            system_profile,
            lock_screen_background,
        }
    }
}

impl<S> ReferenceRuntimeProbe<S, UnknownLockScreenBackgroundProbe> {
    /// Safe default for ordinary switches. Lock-screen recipes remain inapplicable until a
    /// separately verified background probe is injected.
    pub fn with_unknown_lock_screen(system_profile: S) -> Self {
        Self::new(system_profile, UnknownLockScreenBackgroundProbe)
    }
}

impl<S, L> RuntimeProbe for ReferenceRuntimeProbe<S, L>
where
    S: SystemProfileProbe,
    L: LockScreenBackgroundProbe,
{
    fn probe(&self) -> Result<RuntimeFacts, RuntimeProbeError> {
        // Never cache: apply compares this fresh profile with the inspected fingerprint.
        let profile = self
            .system_profile
            .probe()
            .map_err(|error| RuntimeProbeError(format!("system profile probe: {error}")))?;
        let environment = WindowsEnvironment::try_from(profile)
            .map_err(|error| RuntimeProbeError(format!("environment bridge: {error}")))?;
        let lock_screen_background = self.lock_screen_background.probe()?;
        Ok(RuntimeFacts {
            environment,
            lock_screen_background,
        })
    }
}
