use dictation_types::{ErrorKind, Languages, PasteRestoreWaits, UserSettings};
use platform_macos::PlatformConfig;

fn user(waits: PasteRestoreWaits) -> UserSettings {
    let mut settings = UserSettings::new(Languages::parse("ta+fi+sw").unwrap());
    settings.paste_restore_waits = waits;
    settings
}

#[test]
fn a_user_who_has_not_chosen_gets_the_configured_waits() {
    let config = PlatformConfig::bundled().unwrap();
    let tuning = config
        .insertion_for(&user(PasteRestoreWaits::default()))
        .unwrap();
    assert_eq!(tuning, config.insertion);
}

#[test]
fn each_chosen_wait_replaces_only_its_own_configured_value() {
    let config = PlatformConfig::bundled().unwrap();
    let configured = &config.insertion;

    let quiet = config
        .insertion_for(&user(PasteRestoreWaits {
            quiet_after_read_ms: Some(1),
            ..PasteRestoreWaits::default()
        }))
        .unwrap();
    assert_eq!(quiet.restore_quiet_after_read_ms, 1);
    assert_eq!(
        quiet.restore_unread_timeout_ms,
        configured.restore_unread_timeout_ms
    );
    assert_eq!(
        quiet.restore_after_failed_paste_ms,
        configured.restore_after_failed_paste_ms
    );

    let unread = config
        .insertion_for(&user(PasteRestoreWaits {
            unread_timeout_ms: Some(30_000),
            ..PasteRestoreWaits::default()
        }))
        .unwrap();
    assert_eq!(unread.restore_unread_timeout_ms, 30_000);
    assert_eq!(
        unread.restore_quiet_after_read_ms,
        configured.restore_quiet_after_read_ms
    );

    let failed = config
        .insertion_for(&user(PasteRestoreWaits {
            after_failed_paste_ms: Some(1),
            ..PasteRestoreWaits::default()
        }))
        .unwrap();
    assert_eq!(failed.restore_after_failed_paste_ms, 1);
    assert_eq!(
        failed.restore_unread_timeout_ms,
        configured.restore_unread_timeout_ms
    );
}

#[test]
fn a_chosen_wait_of_zero_is_refused_naming_it() {
    let config = PlatformConfig::bundled().unwrap();
    let choices = [
        (
            PasteRestoreWaits {
                quiet_after_read_ms: Some(0),
                ..PasteRestoreWaits::default()
            },
            "restoreQuietAfterReadMs",
        ),
        (
            PasteRestoreWaits {
                unread_timeout_ms: Some(0),
                ..PasteRestoreWaits::default()
            },
            "restoreUnreadTimeoutMs",
        ),
        (
            PasteRestoreWaits {
                after_failed_paste_ms: Some(0),
                ..PasteRestoreWaits::default()
            },
            "restoreAfterFailedPasteMs",
        ),
    ];
    for (waits, key) in choices {
        let err = config.insertion_for(&user(waits)).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Validation);
        assert!(err.message.contains(key), "{}", err.message);
    }
}
