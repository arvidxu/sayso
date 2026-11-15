use audio::AudioConfig;
use configuration::ProductConfig;
use dictation_types::{ErrorKind, Languages, UserSettings};
use serde_json::{Value, json};

const PRODUCT_FILE: &str = include_str!("../../../product.conf.json");

/// The product configuration with the audio values in `audio` overridden, and no recording
/// length decided for new users.
fn config_with_audio(audio: Value) -> AudioConfig {
    let mut file: Value = serde_json::from_str(PRODUCT_FILE).expect("JSON");
    file["dictation"]["overrides"] = json!({ "audio": audio });
    file["dictation"]["initialSettings"]["maxRecordingSecs"] = Value::Null;
    ProductConfig::from_json(&file.to_string())
        .and_then(|product| AudioConfig::from_product(&product))
        .expect("valid")
}

fn user(max_recording_secs: Option<u32>) -> UserSettings {
    let mut settings = UserSettings::new(Languages::parse("ta+fi+sw").unwrap());
    settings.max_recording_secs = max_recording_secs;
    settings
}

#[test]
fn a_user_who_has_not_chosen_gets_the_configured_capture_values() {
    let config = config_with_audio(json!({ "capture": { "maxRecordingMs": 45_000 } }));
    let capture = config.capture_for(&user(None)).unwrap();
    assert_eq!(capture, config.capture);
    assert_eq!(capture.max_recording_ms, 45_000);
}

#[test]
fn a_users_choice_sets_the_longest_recording_in_seconds_and_nothing_else() {
    let config = config_with_audio(json!({}));
    // Shorter and longer than the configured length, up to the longest that can be written.
    for secs in [1u32, 30, 600, 7_200, u32::MAX] {
        let capture = config.capture_for(&user(Some(secs))).unwrap();
        let mut expected = config.capture.clone();
        expected.max_recording_ms = u64::from(secs) * 1_000;
        assert_eq!(capture, expected, "{secs}");
        capture.validate().expect("usable capture values");
    }
}

#[test]
fn a_choice_the_audio_values_cannot_work_with_is_refused_saying_the_choice() {
    // (audio values, choices in seconds that are too short, the shortest that is fine)
    let cases = [
        (json!({}), vec![0], 1),
        (
            json!({ "capture": { "levelWindowMs": 5_000 } }),
            vec![1, 4],
            5,
        ),
        (
            json!({ "voiceDetection": { "minSpeechMs": 7_000 } }),
            vec![1, 6],
            7,
        ),
        (
            json!({ "segmentation": { "quietWindowMs": 7_000 } }),
            vec![1, 6],
            7,
        ),
    ];
    for (audio, too_short, fine) in cases {
        let config = config_with_audio(audio.clone());
        for secs in too_short {
            let err = config
                .capture_for(&user(Some(secs)))
                .err()
                .unwrap_or_else(|| panic!("{secs} s was accepted with {audio}"));
            assert_eq!(err.kind, ErrorKind::Validation, "{secs}: {err}");
            assert!(
                err.message.contains(&secs.to_string()),
                "the error does not say {secs}: {err}"
            );
        }
        config
            .capture_for(&user(Some(fine)))
            .unwrap_or_else(|err| panic!("{fine} s was refused with {audio}: {err}"));
    }
}

fn audio_from(product: &str) -> dictation_types::AppResult<AudioConfig> {
    AudioConfig::from_product(&ProductConfig::from_json(product)?)
}

#[test]
fn a_starting_recording_length_the_audio_values_cannot_work_with_is_refused() {
    let with_length_and_audio = |secs: u32, audio: &Value| {
        let mut file: Value = serde_json::from_str(PRODUCT_FILE).expect("JSON");
        file["dictation"]["initialSettings"]["maxRecordingSecs"] = json!(secs);
        file["dictation"]["overrides"] = json!({ "audio": audio });
        file.to_string()
    };
    let cases = [
        json!({ "voiceDetection": { "padMs": 7_000 } }),
        json!({ "segmentation": { "gapMs": 7_000 } }),
        json!({ "capture": { "levelWindowMs": 7_000 } }),
    ];
    for audio in cases {
        let err = audio_from(&with_length_and_audio(6, &audio))
            .err()
            .unwrap_or_else(|| panic!("6 s was accepted with {audio}"));
        assert_eq!(err.kind, ErrorKind::Validation, "{audio}: {err}");
        assert!(
            err.message
                .contains("dictation.initialSettings.maxRecordingSecs"),
            "{audio}: {err}"
        );

        audio_from(&with_length_and_audio(7, &audio))
            .unwrap_or_else(|err| panic!("7 s was refused with {audio}: {err}"));
    }
}
