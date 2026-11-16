use configuration::{PackageConfig, ProductConfig};
use dictation_types::{AppResult, ErrorKind};
use platform_macos::PlatformConfig;
use serde_json::{Value, json};

const PRODUCT_FILE: &str = include_str!("../../../product.conf.json");

fn read(overrides: Value) -> AppResult<PlatformConfig> {
    let mut file: Value = serde_json::from_str(PRODUCT_FILE).expect("JSON");
    file["dictation"]["overrides"] = overrides;
    ProductConfig::from_json(&file.to_string())?.package()
}

#[test]
fn without_overrides_the_package_runs_with_the_values_it_ships() {
    assert_eq!(PlatformConfig::NAME, "platform-macos");
    let bundled = PlatformConfig::bundled().unwrap();
    assert_eq!(read(json!({})).unwrap(), bundled);
    assert_eq!(read(json!({ "platform-macos": {} })).unwrap(), bundled);
}

#[test]
fn an_override_replaces_the_value_it_names() {
    let config =
        read(json!({ "platform-macos": { "insertion": { "restoreQuietAfterReadMs": 75 } } }))
            .unwrap();
    let bundled = PlatformConfig::bundled().unwrap().insertion;
    assert_eq!(config.insertion.restore_quiet_after_read_ms, 75);
    assert_eq!(
        config.insertion.restore_unread_timeout_ms,
        bundled.restore_unread_timeout_ms
    );
}

#[test]
fn an_override_the_package_does_not_have_or_cannot_run_with_is_refused() {
    let cases = [
        (
            "dictation.overrides.platform-macos.insertion.restoreDelayMs",
            json!({ "platform-macos": { "insertion": { "restoreDelayMs": 75 } } }),
        ),
        (
            "dictation.overrides.platform-macos",
            json!({ "platform-macos": { "insertion": { "restoreUnreadTimeoutMs": -1 } } }),
        ),
    ];
    for (place, overrides) in cases {
        match read(overrides.clone()) {
            Ok(_) => panic!("the overrides {overrides} were accepted"),
            Err(err) => {
                assert_eq!(err.kind, ErrorKind::Validation, "{overrides}: {err}");
                assert!(err.message.contains(place), "{overrides}: {err}");
            }
        }
    }
}
