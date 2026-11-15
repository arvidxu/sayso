use audio::{AudioConfig, BUNDLED_CONFIG};
use dictation_types::ErrorKind;
use serde_json::{Value, json};

fn changed(change: impl FnOnce(&mut Value)) -> String {
    let mut file: Value = serde_json::from_str(BUNDLED_CONFIG).expect("JSON");
    change(&mut file);
    file.to_string()
}

fn rejected(change: impl FnOnce(&mut Value)) -> String {
    let err = AudioConfig::from_json(&changed(change)).expect_err("must be rejected");
    assert_eq!(err.kind, ErrorKind::Validation);
    err.to_string()
}

/// The durations that have to fit into the longest recording, as (group, key).
const DURATIONS: [(&str, &str); 7] = [
    ("voiceDetection", "levelWindowMs"),
    ("voiceDetection", "minSpeechMs"),
    ("voiceDetection", "minSilenceMs"),
    ("voiceDetection", "padMs"),
    ("segmentation", "gapMs"),
    ("segmentation", "quietWindowMs"),
    ("capture", "levelWindowMs"),
];

#[test]
fn no_duration_may_be_longer_than_the_longest_recording() {
    for (group, key) in DURATIONS {
        let message = rejected(|f| {
            f["capture"]["maxRecordingMs"] = json!(1_000);
            f[group][key] = json!(1_001);
        });
        assert!(message.contains(group), "{group}.{key}: {message}");
    }
}

#[test]
fn a_duration_equal_to_the_longest_recording_is_accepted() {
    for (group, key) in DURATIONS {
        let json = changed(|f| {
            f["capture"]["maxRecordingMs"] = json!(1_000);
            f[group][key] = json!(1_000);
        });
        AudioConfig::from_json(&json)
            .unwrap_or_else(|err| panic!("{group}.{key} equal to the limit was refused: {err}"));
    }
}

#[test]
fn every_duration_may_equal_the_longest_recording_at_once() {
    let json = changed(|f| {
        f["capture"]["maxRecordingMs"] = json!(1_000);
        for (group, key) in DURATIONS {
            f[group][key] = json!(1_000);
        }
    });
    AudioConfig::from_json(&json).expect("durations equal to the limit are fine");
}
