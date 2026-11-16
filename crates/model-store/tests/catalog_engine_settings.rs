use dictation_types::{ErrorKind, ModelId};
use model_store::ModelCatalog;
use serde_json::{Value, json};

fn entry_with(settings: Option<Value>) -> Value {
    let mut entry = json!({
        "id": "model-a",
        "name": "A model",
        "kind": "asr",
        "engine": "engine-x",
        "group": null,
        "providedBySystem": false,
        "platforms": ["linux-x64", "windows-arm64"],
        "languages": ["sw", "ko", "fi"],
        "files": [{
            "path": "model.bin",
            "sizeBytes": 10,
            "sha256": "0123456789abcdef".repeat(4),
            "downloadUrls": ["https://example.invalid/model.bin"],
            "unpack": null
        }]
    });
    if let Some(settings) = settings {
        entry["engineSettings"] = settings;
    }
    entry
}

fn catalog_text(settings: Option<Value>) -> String {
    json!({ "models": [entry_with(settings)] }).to_string()
}

fn settings_of(settings: Value) -> Vec<(String, String)> {
    let catalog = ModelCatalog::from_json(&catalog_text(Some(settings))).expect("a valid catalog");
    catalog
        .get(&ModelId::new("model-a"))
        .expect("the entry is there")
        .engine_settings
        .clone()
}

fn assert_refused(what: &str, text: &str) {
    match ModelCatalog::from_json(text) {
        Ok(_) => panic!("a catalog with {what} was accepted"),
        Err(err) => assert_eq!(err.kind, ErrorKind::Validation, "{what}: {err}"),
    }
}

fn pair(name: &str, value: &str) -> (String, String) {
    (name.to_owned(), value.to_owned())
}

#[test]
fn an_entry_without_engine_settings_has_none() {
    let catalog = ModelCatalog::from_json(&catalog_text(None)).expect("a valid catalog");
    assert!(catalog.entries()[0].engine_settings.is_empty());
    assert!(settings_of(json!({})).is_empty());
}

#[test]
fn text_settings_are_kept_as_written() {
    let settings = settings_of(json!({
        "family": "family-one",
        "note": "  spaces stay  ",
        "empty": ""
    }));
    assert!(settings.contains(&pair("family", "family-one")));
    assert!(settings.contains(&pair("note", "  spaces stay  ")));
    assert!(settings.contains(&pair("empty", "")));
    assert_eq!(settings.len(), 3);
}

#[test]
fn number_settings_are_kept_as_text() {
    let settings = settings_of(json!({
        "sampleRateHz": 16000,
        "share": 0.25,
        "offset": -3,
        "none": 0
    }));
    assert!(settings.contains(&pair("sampleRateHz", "16000")));
    assert!(settings.contains(&pair("share", "0.25")));
    assert!(settings.contains(&pair("offset", "-3")));
    assert!(settings.contains(&pair("none", "0")));
}

#[test]
fn true_and_false_settings_are_kept_as_text() {
    let settings = settings_of(json!({ "punctuates": true, "lowercases": false }));
    assert!(settings.contains(&pair("punctuates", "true")));
    assert!(settings.contains(&pair("lowercases", "false")));
}

#[test]
fn settings_can_be_read_back_by_name() {
    let text = catalog_text(Some(json!({
        "family": "family-one",
        "sampleRateHz": 22050,
        "punctuates": true
    })));
    let catalog = ModelCatalog::from_json(&text).unwrap();
    let entry = &catalog.entries()[0];
    assert_eq!(entry.engine_setting("family"), Some("family-one"));
    assert_eq!(entry.engine_setting("sampleRateHz"), Some("22050"));
    assert_eq!(entry.engine_setting("punctuates"), Some("true"));
    assert_eq!(entry.engine_setting("somethingElse"), None);
    assert_eq!(entry.engine_setting("Family"), None);
}

#[test]
fn a_nested_object_is_refused() {
    assert_refused(
        "an object as an engine setting",
        &catalog_text(Some(json!({ "limits": { "seconds": 30 } }))),
    );
    assert_refused(
        "an empty object as an engine setting",
        &catalog_text(Some(json!({ "sampleRateHz": 16000, "limits": {} }))),
    );
}

#[test]
fn a_list_is_refused() {
    assert_refused(
        "a list as an engine setting",
        &catalog_text(Some(json!({ "rates": [8000, 16000] }))),
    );
    assert_refused(
        "an empty list as an engine setting",
        &catalog_text(Some(json!({ "family": "family-one", "rates": [] }))),
    );
}

#[test]
fn engine_settings_that_are_not_an_object_are_refused() {
    for settings in [
        json!("family-one"),
        json!(16000),
        json!(["a", "b"]),
        json!(true),
    ] {
        assert_refused(
            &format!("the engine settings {settings}"),
            &catalog_text(Some(settings)),
        );
    }
}

#[test]
fn the_same_setting_twice_is_refused() {
    // Written by hand: a JSON value built in code cannot hold one name twice.
    for repeated in [
        r#"{ "sampleRateHz": 16000, "sampleRateHz": 8000 }"#,
        r#"{ "family": "family-one", "sampleRateHz": 16000, "family": "family-one" }"#,
        r#"{ "punctuates": true, "punctuates": "true" }"#,
    ] {
        let text = catalog_text(Some(json!("PLACEHOLDER"))).replace("\"PLACEHOLDER\"", repeated);
        assert!(text.contains(repeated), "the settings were put in");
        assert_refused(&format!("the settings {repeated}"), &text);
    }
}

#[test]
fn settings_whose_names_differ_are_all_kept() {
    let settings = settings_of(json!({ "rate": 1, "Rate": 2, "rate2": 3 }));
    assert_eq!(settings.len(), 3);
}
