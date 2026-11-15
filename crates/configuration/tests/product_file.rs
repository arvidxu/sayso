mod support;

use configuration::{PackageConfig, ProductConfig};
use dictation_types::ErrorKind;
use serde_json::{Value, json};
use support::{KNOWN, Other, PRODUCT_FILE, Sample, bundled};

fn product() -> Value {
    serde_json::from_str(PRODUCT_FILE).expect("the product configuration is JSON")
}

/// The product file with one change made to its dictation section.
fn changed(change: impl FnOnce(&mut Value)) -> String {
    let mut file = product();
    change(&mut file["dictation"]);
    file.to_string()
}

fn rejected(change: impl FnOnce(&mut Value)) -> String {
    let err = ProductConfig::from_json(&changed(change)).expect_err("must be rejected");
    assert_eq!(err.kind, ErrorKind::Validation);
    err.to_string()
}

#[test]
fn the_shipped_file_loads() {
    ProductConfig::from_json(PRODUCT_FILE).expect("valid product configuration");
}

#[test]
fn the_shipped_file_overrides_only_objects() {
    let file = product();
    let overrides = file["dictation"]["overrides"]
        .as_object()
        .expect("the product file has overrides");
    for (name, value) in overrides {
        assert!(value.is_object(), "{name}");
    }
}

#[test]
fn without_overrides_a_package_runs_with_the_values_it_ships() {
    let config = ProductConfig::from_json(&changed(|d| d["overrides"] = json!({}))).expect("valid");
    assert_eq!(config.package::<Sample>().unwrap(), bundled::<Sample>());
    assert_eq!(config.package::<Other>().unwrap(), bundled::<Other>());

    // An override that names no value is the same as no override.
    let all = changed(|d| d["overrides"] = json!({ "sample": {}, "other": {} }));
    let all = ProductConfig::from_json(&all).unwrap();
    assert_eq!(all.package::<Sample>().unwrap(), bundled::<Sample>());
    assert_eq!(all.package::<Other>().unwrap(), bundled::<Other>());
}

#[test]
fn a_package_file_that_is_not_valid_is_an_error_naming_the_package() {
    struct Broken;
    impl PackageConfig for Broken {
        const NAME: &'static str = "broken";
        const BUNDLED: &'static str = r#"{ "a": 1, "a": 2 }"#;
        fn from_json(_: &str) -> dictation_types::AppResult<Self> {
            Ok(Self)
        }
    }
    let config = ProductConfig::from_json(PRODUCT_FILE).unwrap();
    let Err(err) = config.package::<Broken>() else {
        panic!("a key written twice in the package's own file was accepted");
    };
    assert_eq!(err.kind, ErrorKind::Internal, "{err}");
    assert!(err.message.contains("broken"), "{err}");

    // The package's own checks, with no overrides, name the package's file.
    let config = ProductConfig::from_json(&changed(|d| d["overrides"] = json!({}))).unwrap();
    struct Refusing;
    impl PackageConfig for Refusing {
        const NAME: &'static str = "refusing";
        const BUNDLED: &'static str = "{}";
        fn from_json(_: &str) -> dictation_types::AppResult<Self> {
            Err(dictation_types::AppError::validation("no values"))
        }
    }
    let Err(err) = config.package::<Refusing>() else {
        panic!("accepted");
    };
    assert!(err.message.contains("refusing package ships with"), "{err}");
}

#[test]
fn a_name_the_dictation_section_does_not_have_is_refused() {
    // (the name, the group of the dictation section it is written in, its value)
    let cases = [
        ("somethingNew", None, json!({})),
        ("Overrides", None, json!({})),
        // A package's values belong under `overrides`, not next to it.
        ("sample", None, json!({ "share": 0.25 })),
        ("languages", Some("initialSettings"), json!("pt+it")),
    ];
    for (name, group, value) in cases {
        let message = rejected(|d| match group {
            Some(group) => d[group][name] = value,
            None => d[name] = value,
        });
        assert!(message.contains(name), "{name}: {message}");
    }
}

#[test]
fn overrides_of_a_package_there_is_none_of_are_refused_when_names_are_checked() {
    for name in ["video", "sample_one", "Sample", "timing"] {
        let product = changed(|d| d["overrides"] = json!({ "sample": {}, name: {} }));
        let config =
            ProductConfig::from_json(&product).unwrap_or_else(|err| panic!("{name}: {err}"));
        // Every package can still read its own values.
        config.package::<Sample>().expect("sample");
        let err = config
            .check_package_names(&KNOWN)
            .expect_err("unknown package");
        assert_eq!(err.kind, ErrorKind::Validation, "{name}: {err}");
        assert!(
            err.message.contains(&format!("dictation.overrides.{name}")),
            "{name}: {err}"
        );
    }
    let product = changed(|d| d["overrides"] = json!({ "sample": {}, "other": null }));
    let config = ProductConfig::from_json(&product).unwrap();
    config.check_package_names(&KNOWN).expect("known");
    config
        .check_package_names(&[Sample::NAME])
        .expect("a package written as null has no overrides");
}

#[test]
fn missing_overrides_or_initial_settings_are_refused() {
    for key in ["overrides", "initialSettings"] {
        let message = rejected(|d| {
            d.as_object_mut().unwrap().remove(key);
        });
        assert!(message.contains(key), "{key}: {message}");
    }
}

#[test]
fn overrides_or_initial_settings_that_are_not_an_object_are_refused() {
    for value in [Value::Null, json!([]), json!("none"), json!(0)] {
        rejected(|d| d["overrides"] = value.clone());
        rejected(|d| d["initialSettings"] = value.clone());
    }
}

#[test]
fn text_that_is_not_a_product_file_is_an_error() {
    for text in [
        "",
        "not json",
        "[]",
        "{}",
        r#"{"dictation": 3}"#,
        r#"{"dictation": {}}"#,
        r#"{"overrides": {}, "initialSettings": {}}"#,
    ] {
        let err = ProductConfig::from_json(text).expect_err(text);
        assert_eq!(err.kind, ErrorKind::Validation, "{text:?}");
    }
}
