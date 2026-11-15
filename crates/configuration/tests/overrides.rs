mod support;

use configuration::ProductConfig;
use dictation_types::ErrorKind;
use serde_json::{Value, json};
use support::{Other, Sample, bundled, product_with, read, refused};

/// Package `Sample` read with `overrides` as the product's overrides.
fn sample(overrides: Value) -> Sample {
    read::<Sample>(overrides.clone(), &[])
        .unwrap_or_else(|err| panic!("the overrides {overrides} were refused: {err}"))
}

fn assert_refused_at(place: &str, overrides: Value) {
    let message = refused::<Sample>(overrides.clone(), &[]);
    assert!(message.contains(place), "{overrides}: {message}");
}

#[test]
fn an_override_replaces_only_the_value_it_names() {
    type Change = fn(&mut Sample);
    let cases: [(Value, Change); 6] = [
        (
            json!({ "sample": { "timing": { "timeoutMs": 35 } } }),
            |c| {
                c.timing.timeout_ms = 35;
            },
        ),
        (json!({ "sample": { "share": 0.125 } }), |c| c.share = 0.125),
        (json!({ "sample": { "label": "two" } }), |c| {
            c.label = "two".to_owned();
        }),
        // A value that is null in the package's file may be given any kind.
        (json!({ "sample": { "fallback": "elsewhere" } }), |c| {
            c.fallback = Some("elsewhere".to_owned());
        }),
        (
            json!({ "sample": { "items": [ { "path": "c", "weight": 7 } ] } }),
            |c| {
                c.items.truncate(1);
                c.items[0].path = "c".to_owned();
                c.items[0].weight = 7;
            },
        ),
        // Several groups of one package and several packages at once.
        (
            json!({
                "sample": { "timing": { "timeoutMs": 40, "windowMs": 20 }, "share": 0.25 },
                "other": { "level": 9 }
            }),
            |c| {
                c.timing.timeout_ms = 40;
                c.timing.window_ms = 20;
                c.share = 0.25;
            },
        ),
    ];
    let reference = bundled::<Sample>();
    for (overrides, change) in cases {
        let mut expected = reference.clone();
        change(&mut expected);
        assert_ne!(expected, reference, "{overrides} changes something");
        assert_eq!(sample(overrides.clone()), expected, "{overrides}");
    }
}

#[test]
fn a_package_reads_only_its_own_overrides() {
    let overrides = json!({ "sample": { "share": 0.25 }, "other": { "level": 9 } });
    assert_eq!(read::<Other>(overrides.clone(), &[]).unwrap().level, 9);
    assert_eq!(sample(overrides).share, 0.25);
    assert_eq!(
        sample(json!({ "other": { "level": 9 } })),
        bundled::<Sample>()
    );

    // An override one package cannot run with does not stop another from reading its own.
    let broken_other = json!({ "other": { "level": 0, "depth": 1 } });
    assert_eq!(sample(broken_other.clone()), bundled::<Sample>());
    refused::<Other>(broken_other, &[]);
}

#[test]
fn an_override_written_as_null_is_no_override() {
    assert_eq!(sample(json!({ "sample": null })), bundled::<Sample>());
    assert_eq!(sample(json!({})), bundled::<Sample>());
}

#[test]
fn a_value_the_package_does_not_have_is_refused_with_its_full_place() {
    let cases = [
        (
            "dictation.overrides.sample.timing.timeoutMS",
            json!({ "sample": { "timing": { "timeoutMS": 30 } } }),
        ),
        (
            "dictation.overrides.sample.playback",
            json!({ "sample": { "playback": { "volume": 1 } } }),
        ),
        // A value of a group, written one level too high.
        (
            "dictation.overrides.sample.timeoutMs",
            json!({ "sample": { "timeoutMs": 30 } }),
        ),
        // Next to good overrides.
        (
            "dictation.overrides.sample.timing.window",
            json!({
                "sample": { "timing": { "timeoutMs": 30, "window": 10 } },
                "other": { "level": 2 }
            }),
        ),
    ];
    for (place, overrides) in cases {
        assert_refused_at(place, overrides);
    }
}

#[test]
fn an_override_of_another_kind_than_the_value_it_replaces_is_refused_with_both_kinds() {
    let cases = [
        (
            "dictation.overrides.sample.timing.timeoutMs",
            json!({ "sample": { "timing": { "timeoutMs": "long" } } }),
            ["number", "text"],
        ),
        (
            "dictation.overrides.sample.timing.timeoutMs",
            json!({ "sample": { "timing": { "timeoutMs": true } } }),
            ["number", "true"],
        ),
        (
            "dictation.overrides.sample.share",
            json!({ "sample": { "share": null } }),
            ["number", "null"],
        ),
        (
            "dictation.overrides.sample.timing",
            json!({ "sample": { "timing": 20 } }),
            ["object", "number"],
        ),
        (
            "dictation.overrides.sample.items",
            json!({ "sample": { "items": {} } }),
            ["list", "object"],
        ),
        (
            "dictation.overrides.sample.timing",
            json!({ "sample": { "timing": [] } }),
            ["object", "list"],
        ),
        (
            "dictation.overrides.sample.label",
            json!({ "sample": { "label": 3 } }),
            ["text", "number"],
        ),
    ];
    for (place, overrides, kinds) in cases {
        let message = refused::<Sample>(overrides.clone(), &[]);
        assert!(message.contains(place), "{overrides}: {message}");
        for kind in kinds {
            assert!(
                message.contains(kind),
                "{overrides}: no '{kind}' in {message}"
            );
        }
    }
}

#[test]
fn an_override_that_is_not_an_object_is_refused_naming_the_package() {
    for package in ["sample", "other", "anything"] {
        for value in [json!(5), json!("fast"), json!([]), json!([{}]), json!(true)] {
            let overrides = json!({ package: value });
            // Refused as the product file is read, before any package reads its values.
            let err =
                ProductConfig::from_json(&product_with(overrides.clone())).expect_err("refused");
            assert_eq!(err.kind, ErrorKind::Validation, "{overrides}: {err}");
            assert!(
                err.message
                    .contains(&format!("dictation.overrides.{package}")),
                "{overrides}: {err}"
            );
        }
    }
}

/// Each package checks its own values; what is checked here is that an override goes
/// through those checks and the error names the package's overrides.
#[test]
fn an_override_the_package_cannot_run_with_is_refused_naming_the_package() {
    let cases = [
        // Outside the range of the value.
        json!({ "sample": { "timing": { "timeoutMs": 0 } } }),
        json!({ "sample": { "share": 1.5 } }),
        // A number where the package reads a whole one.
        json!({ "sample": { "timing": { "timeoutMs": 1.5 } } }),
        // A list, which replaces the whole list, of something else than the package lists.
        json!({ "sample": { "items": ["a/b"] } }),
        // In range by itself, but not together with the values the package ships.
        json!({ "sample": { "timing": { "windowMs": 40 } } }),
    ];
    for overrides in cases {
        assert_refused_at("dictation.overrides.sample", overrides);
    }
    let message = refused::<Other>(json!({ "other": { "level": 0 } }), &[]);
    assert!(message.contains("dictation.overrides.other"), "{message}");
}

/// The product file with `overrides_text` written by hand as its overrides: a JSON value
/// built in code cannot hold one name twice.
fn product_text_with(overrides_text: &str) -> String {
    let text = product_with(json!("PLACEHOLDER")).replace("\"PLACEHOLDER\"", overrides_text);
    assert!(text.contains(overrides_text), "the overrides were put in");
    text
}

/// Package `Sample` read from `product`.
fn sample_from(product: &str) -> Result<Sample, dictation_types::AppError> {
    ProductConfig::from_json(product)?.package::<Sample>()
}

#[test]
fn a_key_written_twice_in_an_override_is_refused() {
    for twice in [
        r#"{ "sample": { "timing": { "timeoutMs": 30, "timeoutMs": 40 } } }"#,
        r#"{ "sample": { "timing": { "timeoutMs": 30 }, "timing": { "windowMs": 5 } } }"#,
        r#"{ "sample": { "share": 0.25, "share": 0.25 } }"#,
        r#"{ "sample": {}, "sample": {} }"#,
        r#"{ "sample": { "items": [
            { "path": "", "path": "a", "weight": 1 }
        ] } }"#,
    ] {
        match sample_from(&product_text_with(twice)) {
            Ok(_) => panic!("{twice} was accepted"),
            Err(err) => assert_eq!(err.kind, ErrorKind::Validation, "{twice}: {err}"),
        }
    }
    // The same written once is accepted.
    for once in [
        r#"{ "sample": { "timing": { "timeoutMs": 40, "windowMs": 40 } } }"#,
        r#"{ "sample": { "share": 0.25 } }"#,
    ] {
        sample_from(&product_text_with(once))
            .unwrap_or_else(|err| panic!("{once} was refused: {err}"));
    }
}
