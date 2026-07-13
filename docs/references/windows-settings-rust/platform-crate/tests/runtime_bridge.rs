use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
};

use deskmakeover_windows_settings_reference::{
    LockScreenBackground, RuntimeProbe, RuntimeProbeError,
};
use dm_windows_settings_platform::{
    CpuArchitecture, LockScreenBackgroundProbe, PackageIdentity, ProfileError,
    ReferenceRuntimeProbe, SystemProfile, SystemProfileProbe, WindowsVersion,
};

#[derive(Debug)]
struct SequenceProfileProbe {
    profiles: RefCell<VecDeque<SystemProfile>>,
    calls: Rc<Cell<usize>>,
}

impl SystemProfileProbe for SequenceProfileProbe {
    fn probe(&self) -> Result<SystemProfile, ProfileError> {
        self.calls.set(self.calls.get() + 1);
        self.profiles
            .borrow_mut()
            .pop_front()
            .ok_or(ProfileError::Version(-1))
    }
}

#[derive(Debug, Clone, Copy)]
struct PictureProbe;

impl LockScreenBackgroundProbe for PictureProbe {
    fn probe(&self) -> Result<LockScreenBackground, RuntimeProbeError> {
        Ok(LockScreenBackground::Picture)
    }
}

fn profile(build: u32, display_version: &str) -> SystemProfile {
    SystemProfile {
        version: WindowsVersion {
            major: 10,
            minor: 0,
            build,
            revision: Some(8_737),
        },
        display_version: display_version.into(),
        edition_id: "Professional".into(),
        installation_type: "Client".into(),
        product_type: Some(48),
        is_workstation: true,
        region: Some("CN".into()),
        native_architecture: CpuArchitecture::X64,
        process_architecture: CpuArchitecture::X64,
        package_identity: PackageIdentity::Unpackaged,
    }
}

#[test]
fn every_runtime_probe_fetches_a_fresh_complete_system_profile() {
    let calls = Rc::new(Cell::new(0));
    let system = SequenceProfileProbe {
        profiles: RefCell::new(VecDeque::from([
            profile(26_100, "24H2"),
            profile(26_200, "25H2"),
        ])),
        calls: calls.clone(),
    };
    let probe = ReferenceRuntimeProbe::with_unknown_lock_screen(system);

    let first = probe.probe().unwrap();
    let second = probe.probe().unwrap();

    assert_eq!(calls.get(), 2);
    assert_eq!(first.environment.build, 26_100);
    assert_eq!(second.environment.build, 26_200);
    assert_eq!(second.environment.display_version, "25H2");
    assert_eq!(first.lock_screen_background, LockScreenBackground::Unknown);
    assert_eq!(second.lock_screen_background, LockScreenBackground::Unknown);
}

#[test]
fn verified_lock_screen_probe_can_be_composed_explicitly() {
    let system = SequenceProfileProbe {
        profiles: RefCell::new(VecDeque::from([profile(26_100, "24H2")])),
        calls: Rc::new(Cell::new(0)),
    };
    let probe = ReferenceRuntimeProbe::new(system, PictureProbe);

    assert_eq!(
        probe.probe().unwrap().lock_screen_background,
        LockScreenBackground::Picture
    );
}
