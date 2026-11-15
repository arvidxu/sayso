mod support;

use configuration::{PackageConfig, ProductConfig};
use dictation_types::ErrorKind;
use serde_json::{Value, json};
use support::{Item, KNOWN, Other, PRODUCT_FILE, Sample, bundled, product_with};

/// Package `T` read from `product` with `layers` on top.
fn layered<T: PackageConfig>(product: &str, layers: &[Value]) -> T {
    let texts: Vec<String> = layers.iter().map(Value::to_string).collect();
    let texts: Vec<&str> = texts.iter().map(String::as_str).collect();
    ProductConfig::from_layers(product, &texts)
        .and_then(|config| config.package::<T>())
        .unwrap_or_else(|err| panic!("the layers {layers:?} were refused: {err}"))
}

/// The message the extra layer `text` is refused with, as the product file is read or as
/// package `Sample` reads its values.
fn rejected(text: &str) -> String {
    let result = ProductConfig::from_layers(PRODUCT_FILE, &[text])
        .and_then(|config| config.package::<Sample>().map(|_| config));
    match result {
        Ok(_) => panic!("the layer {text} was accepted"),
        Err(err) => {
            assert_eq!(err.kind, ErrorKind::Validation, "{text}: {err}");
            err.to_string()
        }
    }
}

#[test]
fn no_layers_or_empty_layers_give_the_result_of_the_product_file_alone() {
    let product = product_with(json!({ "sample": { "share": 0.25 } }));
    let reference: Sample = layered(&product, &[]);
    assert_eq!(reference.share, 0.25);
    assert_eq!(
        layered::<Sample>(&product, &[json!({}), json!({ "sample": {} }), json!({})]),
        reference
    );
    assert_eq!(
        ProductConfig::from_layers(&product, &[]).unwrap(),
        ProductConfig::from_json(&product).unwrap()
    );
}

#[test]
fn a_layer_gives_the_same_result_as_the_same_overrides_in_the_product_file() {
    let overrides = json!({
        "sample": { "timing": { "timeoutMs": 12 }, "items": [] },
        "other": { "level": 9 }
    });
    let in_file = product_with(overrides.clone());
    let in_layer = product_with(json!({}));
    assert_eq!(
        layered::<Sample>(&in_layer, std::slice::from_ref(&overrides)),
        layered::<Sample>(&in_file, &[])
    );
    assert_eq!(
        layered::<Other>(&in_layer, std::slice::from_ref(&overrides)),
        layered::<Other>(&in_file, &[])
    );
}

#[test]
fn layers_are_applied_in_order_each_over_what_came_before() {
    // A later layer wins over an earlier one and over the product file.
    let product = product_with(json!({ "sample": { "timing": { "timeoutMs": 30 } } }));
    let first = json!({ "sample": { "timing": { "timeoutMs": 35 } } });
    let second = json!({ "sample": { "timing": { "timeoutMs": 40 } } });
    for (layers, expected) in [
        (vec![first.clone()], 35),
        (vec![first.clone(), second.clone()], 40),
        (vec![second, first], 35),
    ] {
        assert_eq!(
            layered::<Sample>(&product, &layers).timing.timeout_ms,
            expected,
            "{layers:?}"
        );
    }

    // A value a later layer does not mention keeps what an earlier one gave it.
    let product = product_with(json!({
        "sample": { "timing": { "timeoutMs": 30, "windowMs": 20 } },
        "other": { "level": 5 }
    }));
    let layers = [
        json!({ "sample": { "timing": { "timeoutMs": 35 }, "label": "two" } }),
        json!({ "other": { "level": 6 } }),
        json!({ "sample": { "share": 0.125 } }),
    ];
    let config: Sample = layered(&product, &layers);
    assert_eq!(config.timing.timeout_ms, 35, "the first layer");
    assert_eq!(config.timing.window_ms, 20, "the product file");
    assert_eq!(config.label, "two", "the first layer");
    assert_eq!(config.share, 0.125, "the third layer");
    assert_eq!(
        config.items,
        bundled::<Sample>().items,
        "the package's own file"
    );
    assert_eq!(
        layered::<Other>(&product, &layers).level,
        6,
        "the second layer"
    );

    let plain = ProductConfig::from_json(&product_with(json!({}))).unwrap();
    let texts: Vec<String> = layers.iter().map(Value::to_string).collect();
    let texts: Vec<&str> = texts.iter().map(String::as_str).collect();
    let config = ProductConfig::from_layers(&product, &texts).unwrap();
    assert_eq!(config.initial_settings, plain.initial_settings);
}

#[test]
fn a_list_in_a_layer_replaces_the_whole_list_the_product_file_gave() {
    let product = product_with(json!({ "sample": { "items": [
        { "path": "x", "weight": 1 }, { "path": "y", "weight": 2 }, { "path": "z", "weight": 3 }
    ] } }));
    let shipped: Sample = layered(&product, &[]);
    assert_eq!(
        shipped.items.len(),
        3,
        "the product file gives a longer list"
    );
    let config: Sample = layered(
        &product,
        &[json!({ "sample": { "items": [ { "path": "a/b", "weight": 5 } ] } })],
    );
    let mut expected = shipped.clone();
    expected.items = vec![Item {
        path: "a/b".to_owned(),
        weight: 5,
    }];
    assert_eq!(config, expected);

    let none: Sample = layered(&product, &[json!({ "sample": { "items": [] } })]);
    assert!(none.items.is_empty());
}

#[test]
fn layers_that_only_make_sense_together_are_judged_by_the_result() {
    // Neither value alone fits the package's values, both together do.
    let longer_timeout = json!({ "sample": { "timing": { "timeoutMs": 50 } } });
    let longer_window = json!({ "sample": { "timing": { "windowMs": 45 } } });
    let config: Sample = layered(&product_with(json!({})), &[longer_window, longer_timeout]);
    assert_eq!(config.timing.timeout_ms, 50);
    assert_eq!(config.timing.window_ms, 45);
}

#[test]
fn a_layer_that_is_not_an_overrides_object_is_refused() {
    for text in ["", "not json", "[]", "5", "null", "[{}]"] {
        rejected(text);
    }
}

#[test]
fn a_layer_naming_a_package_there_is_none_of_is_refused_when_names_are_checked() {
    // Shaped like the whole product file, or like a part of it other than the overrides.
    let file: Value = serde_json::from_str(PRODUCT_FILE).unwrap();
    let initial = json!({ "initialSettings": file["dictation"]["initialSettings"] }).to_string();
    let whole = json!({ "dictation": file["dictation"] }).to_string();
    let product = product_with(json!({}));
    for (text, name) in [
        (initial.as_str(), "initialSettings"),
        (whole.as_str(), "dictation"),
        (r#"{ "sample": {}, "video": {} }"#, "video"),
        (r#"{ "sample_one": {} }"#, "sample_one"),
    ] {
        let config = ProductConfig::from_layers(&product, &[text])
            .unwrap_or_else(|err| panic!("{text}: {err}"));
        let err = config.check_package_names(&KNOWN).expect_err(text);
        assert_eq!(err.kind, ErrorKind::Validation, "{text}: {err}");
        assert!(err.message.contains(name), "{name}: {err}");
        assert!(err.message.contains("number 1"), "{name}: {err}");
    }
    let config =
        ProductConfig::from_layers(&product, &[r#"{ "sample": {}, "other": {} }"#]).unwrap();
    config.check_package_names(&KNOWN).expect("known packages");
}

#[test]
fn a_layer_goes_through_the_same_checks_as_the_products_overrides() {
    let message = rejected(&json!({ "sample": { "timing": { "timeoutMS": 30 } } }).to_string());
    assert!(
        message.contains("dictation.overrides.sample.timing.timeoutMS"),
        "{message}"
    );

    let message = rejected(&json!({ "sample": { "share": "half" } }).to_string());
    assert!(
        message.contains("dictation.overrides.sample.share"),
        "{message}"
    );

    rejected(&json!({ "sample": 5 }).to_string());
    rejected(r#"{ "sample": { "share": 0.25, "share": 0.5 } }"#);
    rejected(r#"{ "sample": {}, "sample": {} }"#);

    // A value out of range in the last layer that names it.
    let good = json!({ "sample": { "timing": { "timeoutMs": 35 } } }).to_string();
    let zero = json!({ "sample": { "timing": { "timeoutMs": 0 } } }).to_string();
    let err = ProductConfig::from_layers(PRODUCT_FILE, &[&good, &zero])
        .unwrap()
        .package::<Sample>()
        .expect_err("refused");
    assert_eq!(err.kind, ErrorKind::Validation);
    assert!(err.message.contains("dictation.overrides.sample"), "{err}");
}

#[test]
fn one_refused_layer_refuses_everything_wherever_it_is() {
    let good = json!({ "sample": { "timing": { "timeoutMs": 35 } } }).to_string();
    let misspelt = json!({ "sample": { "timing": { "timeoutMS": 35 } } }).to_string();
    let other = json!({ "sample": { "share": 0.25 } }).to_string();
    for layers in [
        [good.as_str(), misspelt.as_str(), other.as_str()],
        [misspelt.as_str(), good.as_str(), other.as_str()],
        [good.as_str(), other.as_str(), misspelt.as_str()],
    ] {
        let err = ProductConfig::from_layers(PRODUCT_FILE, &layers)
            .unwrap()
            .package::<Sample>()
            .expect_err("refused");
        assert_eq!(err.kind, ErrorKind::Validation);
    }
}
