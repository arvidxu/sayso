use audio::{AudioConfig, BUNDLED_CONFIG};
use dictation_types::ErrorKind;
use serde_json::{Value, json};

fn bundled_json() -> Value {
    serde_json::from_str(BUNDLED_CONFIG).expect("the bundled file is JSON")
}

fn changed(change: impl FnOnce(&mut Value)) -> String {
    let mut file = bundled_json();
    change(&mut file);
    file.to_string()
}

fn rejected(change: impl FnOnce(&mut Value)) -> String {
    let err = AudioConfig::from_json(&changed(change)).expect_err("must be rejected");
    assert_eq!(err.kind, ErrorKind::Validation);
    err.to_string()
}

#[test]
fn the_bundled_file_loads_and_is_valid() {
    AudioConfig::bundled().expect("the bundled configuration loads and is valid");
}

#[test]
fn values_are_those_written_in_the_file() {
    let json = changed(|f| {
        f["voiceDetection"]["speechThreshold"] = json!(0.875);
        f["voiceDetection"]["silenceThreshold"] = json!(0.125);
        f["voiceDetection"]["padMs"] = json!(77);
        f["capture"]["deviceSwitchPollMs"] = json!(15);
        f["capture"]["maxRecordingMs"] = json!(123_456);
        f["resampling"]["blockFrames"] = json!(333);
        f["segmentation"]["gapMs"] = json!(44);
        f["segmentation"]["minPartFractionOfTarget"] = json!(0.25);
        f["segmentation"]["targetFractionOfMax"] = json!(0.75);
    });
    let config = AudioConfig::from_json(&json).expect("valid");
    assert_eq!(config.voice_detection.speech_threshold, 0.875);
    assert_eq!(config.voice_detection.silence_threshold, 0.125);
    assert_eq!(config.voice_detection.pad_ms, 77);
    assert_eq!(config.capture.device_switch_poll_ms, 15);
    assert_eq!(config.capture.max_recording_ms, 123_456);
    assert_eq!(config.resampling.block_frames, 333);
    assert_eq!(config.segmentation.gap_ms, 44);
    assert_eq!(config.segmentation.min_part_fraction_of_target, 0.25);
    assert_eq!(config.segmentation.target_fraction_of_max, 0.75);

    let bundled = AudioConfig::bundled().unwrap();
    assert_eq!(
        config.voice_detection.min_loudness,
        bundled.voice_detection.min_loudness
    );
    assert_eq!(
        config.capture.stream_timeout_ms,
        bundled.capture.stream_timeout_ms
    );
}

#[test]
fn a_missing_value_or_group_is_an_error_that_names_it() {
    let groups = bundled_json().as_object().unwrap().clone();
    assert!(!groups.is_empty());
    for (group, values) in groups {
        for key in values.as_object().unwrap().keys() {
            let message = rejected(|f| {
                f[&group].as_object_mut().unwrap().remove(key);
            });
            assert!(message.contains(key.as_str()), "{group}.{key}: {message}");
        }
        let message = rejected(|f| {
            f.as_object_mut().unwrap().remove(&group);
        });
        assert!(message.contains(group.as_str()), "{group}: {message}");
    }
}

#[test]
fn an_unknown_key_is_an_error_not_ignored() {
    let message = rejected(|f| f["voiceDetection"]["speechTreshold"] = json!(0.9));
    assert!(message.contains("speechTreshold"), "{message}");
    let message = rejected(|f| f["somethingNew"] = json!({}));
    assert!(message.contains("somethingNew"), "{message}");
    for group in ["capture", "resampling", "segmentation"] {
        let message = rejected(|f| f[group]["extra"] = json!(1));
        assert!(message.contains("extra"), "{group}: {message}");
    }
}

#[test]
fn a_value_of_the_wrong_kind_is_an_error() {
    rejected(|f| f["voiceDetection"]["speechThreshold"] = json!("high"));
    rejected(|f| f["voiceDetection"]["minSpeechMs"] = json!(1.5));
    rejected(|f| f["capture"]["streamTimeoutMs"] = json!("20"));
    rejected(|f| f["capture"]["maxRecordingMs"] = Value::Null);
    rejected(|f| f["resampling"]["blockFrames"] = json!(-1));
    rejected(|f| f["resampling"]["blockFrames"] = json!(10.5));
    rejected(|f| f["segmentation"]["gapMs"] = json!(true));
    rejected(|f| f["capture"] = json!(20));
}

#[test]
fn text_that_is_not_a_configuration_is_an_error() {
    for text in ["", "not json", "[]", "{}", "3", r#"{"voiceDetection": 3}"#] {
        let err = AudioConfig::from_json(text).expect_err(text);
        assert_eq!(err.kind, ErrorKind::Validation, "{text:?}");
    }
}

#[test]
fn a_value_outside_its_range_is_an_error_that_names_its_group() {
    let cases = [
        ("voiceDetection", "speechThreshold", json!(1.5)),
        ("voiceDetection", "speechThreshold", json!(-0.1)),
        ("voiceDetection", "minLoudness", json!(-0.1)),
        ("voiceDetection", "minLoudness", json!(2.0)),
        ("capture", "streamTimeoutMs", json!(0)),
        ("capture", "deviceSwitchPollMs", json!(0)),
        ("capture", "reopenIntervalMs", json!(0)),
        ("capture", "levelWindowMs", json!(0)),
        ("capture", "maxRecordingMs", json!(0)),
        ("resampling", "blockFrames", json!(0)),
        ("resampling", "minSampleRateHz", json!(0)),
        ("resampling", "maxSampleRateHz", json!(1)),
        ("segmentation", "minPartFractionOfTarget", json!(0)),
        ("segmentation", "minPartFractionOfTarget", json!(1.2)),
        ("segmentation", "quietWindowMs", json!(0)),
        ("segmentation", "targetFractionOfMax", json!(0)),
        ("segmentation", "targetFractionOfMax", json!(-0.5)),
        ("segmentation", "targetFractionOfMax", json!(1.01)),
    ];
    for (group, key, value) in cases {
        let message = rejected(|f| f[group][key] = value.clone());
        assert!(
            message.contains(group),
            "{group}.{key} = {value}: {message}"
        );
    }
}

#[test]
fn checking_a_configuration_built_in_code_finds_the_same_faults() {
    let good = AudioConfig::bundled().unwrap();

    let mut bad = good.clone();
    bad.capture.stream_timeout_ms = 0;
    let err = bad.validate().expect_err("no timeout");
    assert!(err.message.contains("capture"), "{err}");

    let mut bad = good.clone();
    bad.voice_detection.min_loudness = 3.0;
    let err = bad.validate().expect_err("loudness out of range");
    assert!(err.message.contains("voiceDetection"), "{err}");

    let mut bad = good.clone();
    bad.resampling.block_frames = 0;
    let err = bad.validate().expect_err("no block");
    assert!(err.message.contains("resampling"), "{err}");

    let mut bad = good;
    bad.segmentation.quiet_window_ms = 0;
    let err = bad.validate().expect_err("no window");
    assert!(err.message.contains("segmentation"), "{err}");
}
