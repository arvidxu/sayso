use audio::AudioConfig;
use configuration::{PackageConfig, ProductConfig};
use dictation_types::{AppResult, ErrorKind};
use serde_json::{Value, json};

const PRODUCT_FILE: &str = include_str!("../../../product.conf.json");

fn product_text(overrides: Value) -> String {
    let mut file: Value = serde_json::from_str(PRODUCT_FILE).expect("JSON");
    file["dictation"]["overrides"] = overrides;
    file.to_string()
}

fn read(product: &str, layers: &[&str]) -> AppResult<AudioConfig> {
    AudioConfig::from_product(&ProductConfig::from_layers(product, layers)?)
}

/// The audio values the product file gives with `overrides` as its overrides.
fn with_overrides(overrides: Value) -> AudioConfig {
    read(&product_text(overrides.clone()), &[])
        .unwrap_or_else(|err| panic!("the overrides {overrides} were refused: {err}"))
}

fn rejected(overrides: Value) -> String {
    match read(&product_text(overrides.clone()), &[]) {
        Ok(_) => panic!("the overrides {overrides} were accepted"),
        Err(err) => {
            assert_eq!(err.kind, ErrorKind::Validation, "{overrides}: {err}");
            err.to_string()
        }
    }
}

fn assert_refused_at(place: &str, overrides: Value) {
    let message = rejected(overrides.clone());
    assert!(message.contains(place), "{overrides}: {message}");
}

#[test]
fn the_package_is_overridden_under_its_own_name() {
    assert_eq!(AudioConfig::NAME, "audio");
    assert_eq!(AudioConfig::BUNDLED, audio::BUNDLED_CONFIG);
    let config = with_overrides(json!({ "audio": { "capture": { "streamTimeoutMs": 35 } } }));
    assert_eq!(config.capture.stream_timeout_ms, 35);
}

#[test]
fn without_overrides_the_package_runs_with_the_values_it_ships() {
    let bundled = AudioConfig::bundled().unwrap();
    assert_eq!(with_overrides(json!({})), bundled);
    assert_eq!(with_overrides(json!({ "audio": {} })), bundled);
    assert_eq!(read(PRODUCT_FILE, &[]).unwrap(), bundled);
}

#[test]
fn an_override_replaces_only_the_value_it_names() {
    type Change = fn(&mut AudioConfig);
    let cases: [(Value, Change); 3] = [
        (
            json!({ "audio": { "capture": { "streamTimeoutMs": 35 } } }),
            |c| c.capture.stream_timeout_ms = 35,
        ),
        (
            json!({ "audio": { "voiceDetection": { "minSilenceMs": 1_234 } } }),
            |c| c.voice_detection.min_silence_ms = 1_234,
        ),
        // Several groups at once.
        (
            json!({
                "audio": {
                    "voiceDetection": { "speechThreshold": 0.875, "padMs": 12 },
                    "capture": { "levelWindowMs": 40, "maxRecordingMs": 90_000 },
                    "segmentation": { "minPartFractionOfTarget": 0.25 }
                }
            }),
            |c| {
                c.voice_detection.speech_threshold = 0.875;
                c.voice_detection.pad_ms = 12;
                c.capture.level_window_ms = 40;
                c.capture.max_recording_ms = 90_000;
                c.segmentation.min_part_fraction_of_target = 0.25;
            },
        ),
    ];
    let reference = with_overrides(json!({}));
    for (overrides, change) in cases {
        let mut expected = reference.clone();
        change(&mut expected);
        assert_ne!(expected, reference, "{overrides} changes something");
        assert_eq!(with_overrides(overrides.clone()), expected, "{overrides}");
    }
}

#[test]
fn a_value_the_package_does_not_have_is_refused_with_its_full_place() {
    let cases = [
        (
            "dictation.overrides.audio.capture.streamTimeoutMS",
            json!({ "audio": { "capture": { "streamTimeoutMS": 30 } } }),
        ),
        (
            "dictation.overrides.audio.playback",
            json!({ "audio": { "playback": { "volume": 1 } } }),
        ),
        (
            "dictation.overrides.audio.streamTimeoutMs",
            json!({ "audio": { "streamTimeoutMs": 30 } }),
        ),
    ];
    for (place, overrides) in cases {
        assert_refused_at(place, overrides);
    }
}

#[test]
fn an_override_of_another_kind_than_the_value_it_replaces_is_refused() {
    let cases = [
        (
            "dictation.overrides.audio.capture.streamTimeoutMs",
            json!({ "audio": { "capture": { "streamTimeoutMs": "long" } } }),
        ),
        (
            "dictation.overrides.audio.voiceDetection.speechThreshold",
            json!({ "audio": { "voiceDetection": { "speechThreshold": null } } }),
        ),
        (
            "dictation.overrides.audio.capture",
            json!({ "audio": { "capture": 20 } }),
        ),
    ];
    for (place, overrides) in cases {
        assert_refused_at(place, overrides);
    }
}

#[test]
fn an_override_the_package_cannot_run_with_is_refused_naming_the_package() {
    for overrides in [
        json!({ "audio": { "capture": { "streamTimeoutMs": 0 } } }),
        json!({ "audio": { "capture": { "streamTimeoutMs": 1.5 } } }),
        // In range by itself, but not together with the values the package ships.
        json!({ "audio": { "capture": { "maxRecordingMs": 5 } } }),
    ] {
        assert_refused_at("dictation.overrides.audio", overrides);
    }
}

#[test]
fn layers_are_applied_in_order_and_judged_by_the_result() {
    let product = product_text(json!({ "audio": { "capture": { "streamTimeoutMs": 30 } } }));
    let first = json!({ "audio": { "capture": { "streamTimeoutMs": 35 } } }).to_string();
    let second = json!({ "audio": { "segmentation": { "gapMs": 7 } } }).to_string();
    let config = read(&product, &[&first, &second]).unwrap();
    assert_eq!(config.capture.stream_timeout_ms, 35);
    assert_eq!(config.segmentation.gap_ms, 7);

    // Neither value alone fits the package's values, both together do.
    let lower_speech = json!({ "audio": { "voiceDetection": { "speechThreshold": 0.2 } } });
    let lower_silence = json!({ "audio": { "voiceDetection": { "silenceThreshold": 0.1 } } });
    let config = read(
        &product_text(json!({})),
        &[&lower_speech.to_string(), &lower_silence.to_string()],
    )
    .unwrap();
    assert_eq!(config.voice_detection.speech_threshold, 0.2);
    assert_eq!(config.voice_detection.silence_threshold, 0.1);
}
