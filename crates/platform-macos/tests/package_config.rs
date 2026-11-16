use dictation_types::ErrorKind;
use platform_macos::{BUNDLED_CONFIG, PlatformConfig};
use serde_json::{Value, json};

fn changed(change: impl FnOnce(&mut Value)) -> String {
    let mut file: Value = serde_json::from_str(BUNDLED_CONFIG).unwrap();
    change(&mut file);
    file.to_string()
}

fn rejected(change: impl FnOnce(&mut Value)) -> String {
    let err = PlatformConfig::from_json(&changed(change)).expect_err("must be rejected");
    assert_eq!(err.kind, ErrorKind::Validation);
    err.to_string()
}

#[test]
fn the_bundled_file_loads() {
    let config = PlatformConfig::bundled().unwrap();
    let insertion = config.insertion;
    assert!(insertion.restore_quiet_after_read_ms > 0);
    assert!(insertion.restore_after_failed_paste_ms > 0);
    assert!(insertion.restore_unread_timeout_ms > insertion.restore_quiet_after_read_ms);
}

#[test]
fn missing_unknown_and_wrongly_shaped_values_are_refused() {
    for key in [
        "restoreQuietAfterReadMs",
        "restoreUnreadTimeoutMs",
        "restoreAfterFailedPasteMs",
    ] {
        rejected(|f| {
            f["insertion"].as_object_mut().unwrap().remove(key);
        });
    }
    rejected(|f| f["insertion"]["clipboardRestoreDelayMs"] = json!(200));
    rejected(|f| f["insertion"]["typingSpeed"] = json!(3));
    rejected(|f| f["hotkeys"] = json!({}));
    rejected(|f| f["insertion"]["restoreQuietAfterReadMs"] = json!("200"));
    rejected(|f| f["insertion"] = json!([200]));
    for key in [
        "restoreQuietAfterReadMs",
        "restoreUnreadTimeoutMs",
        "restoreAfterFailedPasteMs",
    ] {
        let message = rejected(|f| f["insertion"][key] = json!(0));
        assert!(message.contains(key), "{message}");
    }
    assert_eq!(
        PlatformConfig::from_json("[]").unwrap_err().kind,
        ErrorKind::Validation
    );
}
