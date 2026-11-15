use dictation_types::{PermissionKind, PermissionReport, PermissionStatus};

#[test]
fn a_permission_nobody_reported_is_not_determined() {
    let report = PermissionReport::new(vec![(
        PermissionKind::Microphone,
        PermissionStatus::Granted,
    )]);
    assert_eq!(
        report.status_of(PermissionKind::Microphone),
        PermissionStatus::Granted
    );
    assert_eq!(
        report.status_of(PermissionKind::Accessibility),
        PermissionStatus::NotDetermined
    );
}

#[test]
fn a_platform_that_requires_no_permission_has_all_it_needs() {
    for report in [
        PermissionReport::default(),
        PermissionReport::new(Vec::new()),
    ] {
        assert!(report.all_granted());
        assert!(report.missing().is_empty());
    }
}

#[test]
fn all_granted_requires_every_listed_permission() {
    let all = PermissionReport::new(vec![
        (PermissionKind::Microphone, PermissionStatus::Granted),
        (PermissionKind::Accessibility, PermissionStatus::Granted),
    ]);
    assert!(all.all_granted());
    assert!(all.missing().is_empty());

    for status in [
        PermissionStatus::Denied,
        PermissionStatus::NotDetermined,
        PermissionStatus::Restricted,
    ] {
        let partial = PermissionReport::new(vec![
            (PermissionKind::Microphone, PermissionStatus::Granted),
            (PermissionKind::InputMonitoring, status),
        ]);
        assert!(!partial.all_granted(), "{status:?}");
        assert_eq!(partial.missing(), [PermissionKind::InputMonitoring]);
        assert!(!status.is_granted());
    }
}
