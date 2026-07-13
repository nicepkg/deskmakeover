use std::{cell::RefCell, error::Error, fmt};

use crate::WindowsEnvironment;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockScreenBackground {
    Unknown,
    Picture,
    Slideshow,
    Spotlight,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeFacts {
    pub environment: WindowsEnvironment,
    pub lock_screen_background: LockScreenBackground,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeProbeError(pub String);

impl fmt::Display for RuntimeProbeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for RuntimeProbeError {}

impl From<String> for RuntimeProbeError {
    fn from(message: String) -> Self {
        Self(message)
    }
}

impl From<&str> for RuntimeProbeError {
    fn from(message: &str) -> Self {
        Self(message.to_owned())
    }
}

pub trait RuntimeProbe {
    fn probe(&self) -> Result<RuntimeFacts, RuntimeProbeError>;
}

#[derive(Debug)]
pub struct MemoryRuntimeProbe {
    state: RefCell<MemoryRuntimeProbeState>,
}

#[derive(Debug)]
struct MemoryRuntimeProbeState {
    facts: RuntimeFacts,
    next_failure: Option<RuntimeProbeError>,
}

impl MemoryRuntimeProbe {
    pub fn new(facts: RuntimeFacts) -> Self {
        Self {
            state: RefCell::new(MemoryRuntimeProbeState {
                facts,
                next_failure: None,
            }),
        }
    }

    pub fn set_facts(&self, facts: RuntimeFacts) {
        self.state.borrow_mut().facts = facts;
    }

    pub fn fail_next(&self, error: impl Into<RuntimeProbeError>) {
        self.state.borrow_mut().next_failure = Some(error.into());
    }
}

impl RuntimeProbe for MemoryRuntimeProbe {
    fn probe(&self) -> Result<RuntimeFacts, RuntimeProbeError> {
        let mut state = self.state.borrow_mut();
        if let Some(error) = state.next_failure.take() {
            return Err(error);
        }

        Ok(state.facts.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WindowsEdition;

    fn facts(build: u32, background: LockScreenBackground) -> RuntimeFacts {
        RuntimeFacts {
            environment: WindowsEnvironment {
                major: 10,
                minor: 0,
                build,
                ubr: 1,
                display_version: "24H2".into(),
                edition_id: "Professional".into(),
                edition: WindowsEdition::Pro,
                installation_type: "Client".into(),
                product_type: 48,
                is_workstation: true,
                region: "US".to_owned(),
                native_architecture: "x64".to_owned(),
                process_architecture: "x64".to_owned(),
                packaged: false,
            },
            lock_screen_background: background,
        }
    }

    #[test]
    fn returns_the_latest_runtime_facts() {
        let initial = facts(26_100, LockScreenBackground::Picture);
        let updated = facts(26_200, LockScreenBackground::Spotlight);
        let probe = MemoryRuntimeProbe::new(initial.clone());

        assert_eq!(probe.probe(), Ok(initial));

        probe.set_facts(updated.clone());

        assert_eq!(probe.probe(), Ok(updated));
    }

    #[test]
    fn fails_only_the_next_probe() {
        let expected = facts(26_100, LockScreenBackground::Slideshow);
        let probe = MemoryRuntimeProbe::new(expected.clone());
        probe.fail_next("runtime probe unavailable");

        assert_eq!(
            probe.probe(),
            Err(RuntimeProbeError("runtime probe unavailable".to_owned()))
        );
        assert_eq!(probe.probe(), Ok(expected));
    }

    #[test]
    fn runtime_probe_error_exposes_its_message() {
        let error = RuntimeProbeError::from("unsupported environment");

        assert_eq!(error.to_string(), "unsupported environment");
        let as_error: &dyn Error = &error;
        assert_eq!(as_error.to_string(), "unsupported environment");
    }
}
