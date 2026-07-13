//! Adapters from concrete platform probes to the transaction reference contracts.

mod environment;
mod registry;
mod runtime;

pub use environment::EnvironmentBridgeError;
pub use registry::{PolicyStateProbe, ReferenceRegistryBackend};
pub use runtime::{
    LockScreenBackgroundProbe, ReferenceRuntimeProbe, UnknownLockScreenBackgroundProbe,
};
