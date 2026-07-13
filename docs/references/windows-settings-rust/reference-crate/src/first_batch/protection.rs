use crate::RegistryAddress;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtectedReason {
    StartRecent,
    CloudStore,
    LocalState,
    TaskbarWidgets,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ProtectedProfile;

impl ProtectedProfile {
    pub(super) fn first_batch() -> Self {
        Self
    }

    pub(super) fn classify(self, address: &RegistryAddress) -> Option<ProtectedReason> {
        classify(address)
    }
}

pub(super) fn classify(address: &RegistryAddress) -> Option<ProtectedReason> {
    let segments = address
        .key
        .split(['\\', '/'])
        .filter(|segment| !segment.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    if segments.iter().any(|segment| segment == "cloudstore") {
        return Some(ProtectedReason::CloudStore);
    }
    if segments.iter().any(|segment| segment == "localstate") {
        return Some(ProtectedReason::LocalState);
    }
    let explorer_advanced = segments.ends_with(&[
        "microsoft".into(),
        "windows".into(),
        "currentversion".into(),
        "explorer".into(),
        "advanced".into(),
    ]);
    if explorer_advanced && address.value.eq_ignore_ascii_case("Start_TrackDocs") {
        return Some(ProtectedReason::StartRecent);
    }
    if explorer_advanced && address.value.eq_ignore_ascii_case("TaskbarDa") {
        return Some(ProtectedReason::TaskbarWidgets);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RegistryHive, RegistryView};

    fn address(view: RegistryView, key: &str, value: &str) -> RegistryAddress {
        RegistryAddress::new(RegistryHive::CurrentUser, view, key, value)
    }

    #[test]
    fn matching_is_case_insensitive_segment_aware_and_view_independent() {
        for view in [
            RegistryView::Native,
            RegistryView::Registry32,
            RegistryView::Registry64,
        ] {
            assert_eq!(
                classify(&address(
                    view,
                    r"SOFTWARE\MICROSOFT\WINDOWS\CURRENTVERSION\EXPLORER\ADVANCED",
                    "start_trackdocs"
                )),
                Some(ProtectedReason::StartRecent)
            );
            assert_eq!(
                classify(&address(view, r"Software\Anything\CloudStore\Cache", "x")),
                Some(ProtectedReason::CloudStore)
            );
            assert_eq!(
                classify(&address(view, r"Software/Packages/App/LocalState", "x")),
                Some(ProtectedReason::LocalState)
            );
            assert_eq!(
                classify(&address(
                    view,
                    r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced",
                    "TASKBARDA"
                )),
                Some(ProtectedReason::TaskbarWidgets)
            );
        }
        assert_eq!(
            classify(&address(RegistryView::Native, "CloudStoreLike", "x")),
            None
        );
    }
}
