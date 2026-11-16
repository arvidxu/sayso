use asr_local::RecognitionTuning;
use configuration::{PackageConfig, ProductConfig};
use dictation_types::{AppResult, ErrorKind};
use serde_json::{Value, json};

const PRODUCT_FILE: &str = include_str!("../../../product.conf.json");

fn read(overrides: Value, layers: &[&str]) -> AppResult<RecognitionTuning> {
    let mut file: Value = serde_json::from_str(PRODUCT_FILE).expect("JSON");
    file["dictation"]["overrides"] = overrides;
    ProductConfig::from_layers(&file.to_string(), layers)?.package()
}

fn assert_refused_at(place: &str, overrides: Value) {
    match read(overrides.clone(), &[]) {
        Ok(_) => panic!("the overrides {overrides} were accepted"),
        Err(err) => {
            assert_eq!(err.kind, ErrorKind::Validation, "{overrides}: {err}");
            assert!(err.message.contains(place), "{overrides}: {err}");
        }
    }
}

#[test]
fn without_overrides_the_package_runs_with_the_values_it_ships() {
    assert_eq!(RecognitionTuning::NAME, "asr-local");
    let bundled = RecognitionTuning::bundled().unwrap();
    assert_eq!(read(json!({}), &[]).unwrap(), bundled);
    assert_eq!(read(json!({ "asr-local": {} }), &[]).unwrap(), bundled);
}

#[test]
fn an_override_replaces_only_the_value_it_names() {
    let reference = RecognitionTuning::bundled().unwrap();
    let config = read(json!({ "asr-local": { "threadShare": 0.125 } }), &[]).unwrap();
    assert_eq!(
        config,
        RecognitionTuning {
            thread_share: 0.125,
            ..reference.clone()
        }
    );
    let config = read(
        json!({ "asr-local": { "executionProvider": "provider-one" } }),
        &[],
    )
    .unwrap();
    assert_eq!(
        config,
        RecognitionTuning {
            execution_provider: "provider-one".to_owned(),
            ..reference
        }
    );
}

#[test]
fn a_value_a_later_layer_does_not_mention_keeps_what_an_earlier_one_gave_it() {
    let later = json!({ "asr-local": { "threadShare": 0.125 } }).to_string();
    let config = read(
        json!({ "asr-local": { "executionProvider": "provider-one" } }),
        &[&later],
    )
    .unwrap();
    assert_eq!(config.thread_share, 0.125);
    assert_eq!(config.execution_provider, "provider-one");
}

#[test]
fn an_override_the_package_does_not_have_or_cannot_run_with_is_refused() {
    let cases = [
        (
            "dictation.overrides.asr-local.threadCount",
            json!({ "asr-local": { "threadCount": 4 } }),
        ),
        (
            "dictation.overrides.asr-local.executionProvider",
            json!({ "asr-local": { "executionProvider": 3 } }),
        ),
        (
            "dictation.overrides.asr-local.threadShare",
            json!({ "asr-local": { "threadShare": "half" } }),
        ),
        (
            "dictation.overrides.asr-local",
            json!({ "asr-local": { "threadShare": 1.5 } }),
        ),
    ];
    for (place, overrides) in cases {
        assert_refused_at(place, overrides);
    }
}
