use asr_local::{BUNDLED_CONFIG, RecognitionTuning};
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
    let err = RecognitionTuning::from_json(&changed(change)).expect_err("must be rejected");
    assert_eq!(err.kind, ErrorKind::Validation);
    err.to_string()
}

#[test]
fn the_bundled_file_loads_and_is_valid() {
    let tuning = RecognitionTuning::bundled().expect("the bundled configuration loads");
    tuning.validate().expect("valid");
    assert!(tuning.threads() >= 1);
}

#[test]
fn values_are_those_written_in_the_file() {
    for (share, provider) in [
        (0.125f32, "provider-one"),
        (0.75, "provider-two"),
        (1.0, "p"),
    ] {
        let json = changed(|f| {
            f["threadShare"] = json!(share);
            f["executionProvider"] = json!(provider);
        });
        let tuning = RecognitionTuning::from_json(&json).expect("valid");
        assert_eq!(tuning.thread_share, share);
        assert_eq!(tuning.execution_provider, provider);
    }
}

#[test]
fn a_missing_value_is_an_error_that_names_it() {
    let keys = bundled_json().as_object().unwrap().clone();
    assert!(!keys.is_empty());
    for key in keys.keys() {
        let message = rejected(|f| {
            f.as_object_mut().unwrap().remove(key);
        });
        assert!(message.contains(key.as_str()), "{key}: {message}");
    }
}

#[test]
fn an_unknown_key_is_an_error_not_ignored() {
    let message = rejected(|f| f["threadCount"] = json!(4));
    assert!(message.contains("threadCount"), "{message}");
    let message = rejected(|f| f["threadshare"] = json!(0.5));
    assert!(message.contains("threadshare"), "{message}");
}

#[test]
fn a_value_of_the_wrong_kind_is_an_error() {
    rejected(|f| f["threadShare"] = json!("half"));
    rejected(|f| f["threadShare"] = json!(true));
    rejected(|f| f["threadShare"] = Value::Null);
    rejected(|f| f["threadShare"] = json!([0.5]));
    rejected(|f| f["executionProvider"] = json!(3));
    rejected(|f| f["executionProvider"] = Value::Null);
    rejected(|f| f["executionProvider"] = json!({ "name": "provider-one" }));
}

#[test]
fn text_that_is_not_a_configuration_is_an_error() {
    for text in ["", "not json", "[]", "{}", "0.5", "\"provider-one\""] {
        let err = RecognitionTuning::from_json(text).expect_err(text);
        assert_eq!(err.kind, ErrorKind::Validation, "{text:?}");
    }
}
