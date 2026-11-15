use configuration::ProductConfig;
use dictation_types::{ErrorKind, InsertMethod, Languages, OutputTiming};
use serde_json::{Value, json};

const PRODUCT_FILE: &str = include_str!("../../../product.conf.json");

/// The starting values that may be left open by writing `null`.
const MAY_BE_OPEN: [&str; 3] = [
    "modelIdleUnloadSecs",
    "audioRetentionDays",
    "maxRecordingSecs",
];

/// The product file with one change made to its initial settings.
fn changed(change: impl FnOnce(&mut Value)) -> String {
    let mut file: Value = serde_json::from_str(PRODUCT_FILE).expect("JSON");
    change(&mut file["dictation"]["initialSettings"]);
    file.to_string()
}

fn rejected(change: impl FnOnce(&mut Value)) -> String {
    let err = ProductConfig::from_json(&changed(change)).expect_err("must be rejected");
    assert_eq!(err.kind, ErrorKind::Validation);
    err.to_string()
}

fn languages(spec: &str) -> Languages {
    Languages::parse(spec).expect("valid languages")
}

#[test]
fn the_initial_settings_are_those_written_in_the_file() {
    let json = changed(|s| {
        s["modelIdleUnloadSecs"] = json!(77);
        s["audioRetentionDays"] = json!(9);
        s["maxRecordingSecs"] = json!(123);
        s["muteOtherAudioWhileRecording"] = json!(true);
    });
    let initial = ProductConfig::from_json(&json).unwrap().initial_settings;
    assert_eq!(initial.model_idle_unload_secs, Some(77));
    assert_eq!(initial.audio_retention_days, Some(9));
    assert_eq!(initial.max_recording_secs, Some(123));
    assert!(initial.mute_other_audio_while_recording);

    let json = changed(|s| s["muteOtherAudioWhileRecording"] = json!(false));
    let initial = ProductConfig::from_json(&json).unwrap().initial_settings;
    assert!(!initial.mute_other_audio_while_recording);

    for method in InsertMethod::ALL {
        let json = changed(|s| s["insertMethod"] = json!(method.as_str()));
        let initial = ProductConfig::from_json(&json).unwrap().initial_settings;
        assert_eq!(initial.insert_method, method);
    }
    for timing in OutputTiming::ALL {
        let json = changed(|s| s["outputTiming"] = json!(timing.as_str()));
        let initial = ProductConfig::from_json(&json).unwrap().initial_settings;
        assert_eq!(initial.output_timing, timing);
    }
}

#[test]
fn a_starting_value_written_as_null_is_left_open() {
    for key in MAY_BE_OPEN {
        let config = ProductConfig::from_json(&changed(|s| s[key] = Value::Null))
            .unwrap_or_else(|err| panic!("{key} left open was refused: {err}"));
        let initial = config.initial_settings;
        let open = [
            initial.model_idle_unload_secs.is_none(),
            initial.audio_retention_days.is_none(),
            initial.max_recording_secs.is_none(),
        ];
        let expected = MAY_BE_OPEN.map(|k| k == key);
        assert_eq!(open, expected, "{key}");
    }
}

#[test]
fn every_starting_value_has_to_be_written_even_one_that_may_be_left_open() {
    let file: Value = serde_json::from_str(PRODUCT_FILE).unwrap();
    let keys: Vec<String> = file["dictation"]["initialSettings"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    assert!(MAY_BE_OPEN.iter().all(|key| keys.iter().any(|k| k == key)));
    for key in keys {
        let message = rejected(|s| {
            s.as_object_mut().unwrap().remove(&key);
        });
        assert!(message.contains(&key), "{key}: {message}");
    }
}

#[test]
fn a_starting_value_of_zero_is_refused_naming_the_initial_settings() {
    for key in MAY_BE_OPEN {
        let message = rejected(|s| s[key] = json!(0));
        assert!(
            message.contains("dictation.initialSettings"),
            "{key}: {message}"
        );
        assert!(message.contains(key), "{key}: {message}");
    }
}

#[test]
fn an_initial_setting_of_the_wrong_kind_is_refused() {
    let cases = [
        ("modelIdleUnloadSecs", json!("never")),
        ("modelIdleUnloadSecs", json!(-1)),
        ("audioRetentionDays", json!(1.5)),
        ("maxRecordingSecs", json!("ten minutes")),
        ("outputTiming", json!(1)),
        ("insertMethod", json!(true)),
        ("muteOtherAudioWhileRecording", json!("no")),
        // These cannot be left open.
        ("outputTiming", Value::Null),
        ("insertMethod", Value::Null),
        ("muteOtherAudioWhileRecording", Value::Null),
    ];
    for (key, value) in cases {
        rejected(|s| s[key] = value);
    }
}

#[test]
fn an_unknown_insert_method_or_output_timing_is_refused_naming_the_initial_settings() {
    let cases = [
        ("insertMethod", "telepathy"),
        ("insertMethod", ""),
        // A known one, spelt another way.
        ("insertMethod", "paste_with_clipboard_restore"),
        ("outputTiming", "whenever"),
        ("outputTiming", ""),
        ("outputTiming", "at_the_end"),
    ];
    for (key, value) in cases {
        let message = rejected(|s| s[key] = json!(value));
        assert!(
            message.contains("dictation.initialSettings"),
            "{key} {value:?}: {message}"
        );
        assert!(message.contains(value), "{key} {value:?}: {message}");
    }
}

#[test]
fn a_new_user_starts_with_the_configured_values_and_the_languages_the_app_supplies() {
    for timing in OutputTiming::ALL {
        let json = changed(|s| {
            s["modelIdleUnloadSecs"] = Value::Null;
            s["audioRetentionDays"] = json!(3);
            s["maxRecordingSecs"] = json!(45);
            s["outputTiming"] = json!(timing.as_str());
            s["muteOtherAudioWhileRecording"] = json!(true);
        });
        let config = ProductConfig::from_json(&json).unwrap();
        let initial = &config.initial_settings;
        for spec in ["ko", "ru+fr+en+ja"] {
            let settings = initial.for_new_user(languages(spec));
            assert_eq!(
                settings.languages,
                languages(spec),
                "the configuration never picks languages"
            );
            assert_eq!(settings.model_idle_unload_secs, None);
            assert_eq!(settings.audio_retention_days, Some(3));
            assert_eq!(settings.max_recording_secs, Some(45));
            assert_eq!(settings.output_timing, Some(timing));
            assert_eq!(settings.insert_method, Some(initial.insert_method));
            assert!(settings.mute_other_audio_while_recording);
            assert_eq!(settings.asr_model_id, None);
            assert_eq!(settings.asr_provider_id, None);
            assert_eq!(settings.active_mode_id, None);
        }
    }
}
