use dictation_types::ErrorKind;
use model_store::{BUNDLED_CONFIG, LocalCacheLocation, ModelsConfig};
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
    let err = ModelsConfig::from_json(&changed(change)).expect_err("must be rejected");
    assert_eq!(err.kind, ErrorKind::Validation);
    err.to_string()
}

fn one_cache(env_var: Value, sub_path: &str, file_pattern: &str) -> Value {
    json!([{ "envVar": env_var, "subPath": sub_path, "filePattern": file_pattern }])
}

#[test]
fn the_bundled_file_loads_and_is_valid() {
    ModelsConfig::bundled().expect("the bundled configuration loads and is valid");
}

#[test]
fn values_are_those_written_in_the_file() {
    let json = changed(|f| {
        f["download"]["connectTimeoutMs"] = json!(1234);
        f["download"]["readTimeoutMs"] = json!(5678);
        f["download"]["bufferBytes"] = json!(910);
        f["download"]["attemptsPerAddress"] = json!(7);
        f["download"]["retryDelayMs"] = json!(0);
        f["download"]["unpackedSizeRatio"] = json!(2.25);
        f["localCaches"] = json!([
            { "envVar": "SOME_CACHE", "subPath": "a/b", "filePattern": "*/{sha256}.bin" },
            { "envVar": null, "subPath": "", "filePattern": "{sha256}" }
        ]);
    });
    let config = ModelsConfig::from_json(&json).expect("valid");
    assert_eq!(config.download.connect_timeout_ms, 1234);
    assert_eq!(config.download.read_timeout_ms, 5678);
    assert_eq!(config.download.buffer_bytes, 910);
    assert_eq!(config.download.attempts_per_address, 7);
    assert_eq!(config.download.retry_delay_ms, 0);
    assert_eq!(config.download.unpacked_size_ratio, 2.25);
    assert_eq!(
        config.local_caches,
        [
            LocalCacheLocation {
                env_var: Some("SOME_CACHE".to_owned()),
                sub_path: "a/b".to_owned(),
                file_pattern: "*/{sha256}.bin".to_owned(),
            },
            LocalCacheLocation {
                env_var: None,
                sub_path: String::new(),
                file_pattern: "{sha256}".to_owned(),
            },
        ]
    );
}

#[test]
fn a_missing_value_or_group_is_an_error_that_names_it() {
    let download = bundled_json()["download"].as_object().unwrap().clone();
    assert!(!download.is_empty());
    for key in download.keys() {
        let message = rejected(|f| {
            f["download"].as_object_mut().unwrap().remove(key);
        });
        assert!(message.contains(key.as_str()), "{key}: {message}");
    }
    for group in ["download", "localCaches"] {
        let message = rejected(|f| {
            f.as_object_mut().unwrap().remove(group);
        });
        assert!(message.contains(group), "{group}: {message}");
    }
    for key in ["subPath", "filePattern"] {
        let message = rejected(|f| {
            let mut cache = json!({
                "envVar": "SOME_CACHE", "subPath": "", "filePattern": "{sha256}"
            });
            cache.as_object_mut().unwrap().remove(key);
            f["localCaches"] = json!([cache]);
        });
        assert!(message.contains(key), "{key}: {message}");
    }
}

#[test]
fn an_unknown_key_is_an_error_not_ignored() {
    let message = rejected(|f| f["download"]["retries"] = json!(3));
    assert!(message.contains("retries"), "{message}");
    let message = rejected(|f| f["mirrors"] = json!([]));
    assert!(message.contains("mirrors"), "{message}");
    let message = rejected(|f| {
        f["localCaches"] = json!([{
            "envVar": null, "subPath": "", "filePattern": "{sha256}", "priority": 1
        }]);
    });
    assert!(message.contains("priority"), "{message}");
}

#[test]
fn a_value_of_the_wrong_kind_is_an_error() {
    rejected(|f| f["download"]["connectTimeoutMs"] = json!("soon"));
    rejected(|f| f["download"]["readTimeoutMs"] = json!(-1));
    rejected(|f| f["download"]["bufferBytes"] = json!(1.5));
    rejected(|f| f["download"]["attemptsPerAddress"] = Value::Null);
    rejected(|f| f["download"]["retryDelayMs"] = json!(true));
    rejected(|f| f["download"]["unpackedSizeRatio"] = json!("twice"));
    rejected(|f| f["download"] = json!([1, 2, 3]));
    rejected(|f| f["localCaches"] = json!({}));
    rejected(|f| f["localCaches"] = json!(["a/b"]));
    rejected(|f| f["localCaches"] = one_cache(json!(3), "", "{sha256}"));
}

#[test]
fn text_that_is_not_a_configuration_is_an_error() {
    for text in ["", "not json", "[]", "{}", "3", r#"{"download": 3}"#] {
        let err = ModelsConfig::from_json(text).expect_err(text);
        assert_eq!(err.kind, ErrorKind::Validation, "{text:?}");
    }
}

#[test]
fn a_zero_download_value_is_an_error_that_names_the_group() {
    for key in [
        "connectTimeoutMs",
        "readTimeoutMs",
        "bufferBytes",
        "attemptsPerAddress",
        "unpackedSizeRatio",
    ] {
        let message = rejected(|f| f["download"][key] = json!(0));
        assert!(message.contains("download"), "{key}: {message}");
    }
    let message = rejected(|f| f["download"]["unpackedSizeRatio"] = json!(-1.5));
    assert!(message.contains("download"), "{message}");
}

#[test]
fn not_waiting_between_tries_is_fine() {
    let config =
        ModelsConfig::from_json(&changed(|f| f["download"]["retryDelayMs"] = json!(0))).unwrap();
    assert_eq!(config.download.retry_delay_ms, 0);
}

#[test]
fn a_cache_location_that_makes_no_sense_is_an_error_that_names_the_group() {
    let bad = [
        (
            "a blank variable name",
            one_cache(json!(""), "", "{sha256}"),
        ),
        (
            "a blank variable name",
            one_cache(json!("  "), "", "{sha256}"),
        ),
        (
            "a sub path leaving the folder",
            one_cache(json!("SOME_CACHE"), "../elsewhere", "{sha256}"),
        ),
        (
            "an absolute sub path",
            one_cache(json!("SOME_CACHE"), "/elsewhere", "{sha256}"),
        ),
        (
            "a pattern without the checksum",
            one_cache(json!("SOME_CACHE"), "", "*/blobs/file"),
        ),
        (
            "a pattern with the checksum twice",
            one_cache(json!("SOME_CACHE"), "", "{sha256}/{sha256}"),
        ),
        (
            "a pattern leaving the folder",
            one_cache(json!("SOME_CACHE"), "", "../{sha256}"),
        ),
        (
            "an absolute pattern",
            one_cache(json!("SOME_CACHE"), "", "/abs/{sha256}"),
        ),
        ("an empty pattern", one_cache(json!("SOME_CACHE"), "", "")),
    ];
    for (what, caches) in bad {
        let message = rejected(|f| f["localCaches"] = caches.clone());
        assert!(message.contains("localCaches"), "{what}: {message}");
    }
}

#[test]
fn one_bad_cache_location_among_good_ones_is_still_an_error() {
    let message = rejected(|f| {
        f["localCaches"] = json!([
            { "envVar": "SOME_CACHE", "subPath": "a", "filePattern": "*/{sha256}" },
            { "envVar": null, "subPath": "", "filePattern": "no-checksum" },
            { "envVar": null, "subPath": "b", "filePattern": "{sha256}" }
        ]);
    });
    assert!(message.contains("localCaches"), "{message}");
}

#[test]
fn checking_a_configuration_built_in_code_finds_the_same_faults() {
    let good = ModelsConfig::bundled().unwrap();

    let mut bad = good.clone();
    bad.download.buffer_bytes = 0;
    let err = bad.validate().expect_err("no buffer");
    assert!(err.message.contains("download"), "{err}");

    let mut bad = good;
    bad.local_caches.push(LocalCacheLocation {
        env_var: None,
        sub_path: String::new(),
        file_pattern: "no-checksum".to_owned(),
    });
    let err = bad.validate().expect_err("no checksum in the pattern");
    assert!(err.message.contains("localCaches"), "{err}");
}

#[test]
fn a_cache_location_written_as_a_list_of_values_is_an_error() {
    // Values in the order the bundled file names them, without the names.
    let json = changed(|f| f["localCaches"] = json!([["SOME_CACHE", "", "{sha256}"]]));
    assert!(
        ModelsConfig::from_json(&json).is_err(),
        "a cache location was read from values without names"
    );
}

#[test]
fn the_package_looks_in_no_other_tools_folders_unless_told_to() {
    assert!(ModelsConfig::bundled().unwrap().local_caches.is_empty());
    assert_eq!(bundled_json()["localCaches"], json!([]));
}
