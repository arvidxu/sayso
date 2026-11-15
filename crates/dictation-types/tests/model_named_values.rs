mod support;

use dictation_types::{
    ErrorKind, ModelDescriptor, ModelFile, ModelId, ModelKind, PlatformTarget, ProviderId,
};
use support::lang;

fn pairs(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect()
}

fn model(named_paths: &[(&str, &str)], engine_settings: &[(&str, &str)]) -> ModelDescriptor {
    ModelDescriptor {
        id: ModelId::new("model-a"),
        name: "Model A".to_owned(),
        kind: ModelKind::Asr,
        engine: ProviderId::new("engine-x"),
        group: None,
        files: vec![ModelFile {
            path: "weights.bin".to_owned(),
            size_bytes: 10,
            sha256: "0123456789abcdef".repeat(4),
            download_urls: vec!["https://example.invalid/weights.bin".to_owned()],
            unpack: None,
        }],
        provided_by_system: false,
        named_paths: pairs(named_paths),
        engine_settings: pairs(engine_settings),
        supported_platforms: vec![PlatformTarget::LinuxArm64, PlatformTarget::WindowsX64],
        supported_languages: vec![lang("sw"), lang("ko"), lang("fi")],
    }
}

fn assert_refused(what: &str, model: &ModelDescriptor) {
    match model.validate() {
        Ok(()) => panic!("an entry with {what} was accepted"),
        Err(err) => assert_eq!(err.kind, ErrorKind::Validation, "{what}: {err}"),
    }
}

#[test]
fn an_engine_setting_is_found_by_its_name() {
    let entry = model(
        &[],
        &[("rate", "22050"), ("kind", "kind-one"), ("empty", "")],
    );
    entry.validate().expect("valid");
    assert_eq!(entry.engine_setting("rate"), Some("22050"));
    assert_eq!(entry.engine_setting("kind"), Some("kind-one"));
    assert_eq!(entry.engine_setting("empty"), Some(""));
}

#[test]
fn an_engine_setting_that_was_not_given_is_not_found() {
    let entry = model(&[("rate", "sub/rate.bin")], &[("rate", "22050")]);
    for name in ["Rate", "rat", "rate ", " rate", "rates", "kind", ""] {
        assert_eq!(entry.engine_setting(name), None, "{name:?}");
    }
    assert_eq!(model(&[], &[]).engine_setting("rate"), None);
}

#[test]
fn a_named_path_is_found_by_its_name() {
    let entry = model(
        &[("first", "sub/first.bin"), ("words", "sub/words")],
        &[("first", "not a path")],
    );
    entry.validate().expect("valid");
    assert_eq!(entry.named_path("first"), Some("sub/first.bin"));
    assert_eq!(entry.named_path("words"), Some("sub/words"));
}

#[test]
fn a_named_path_that_was_not_given_is_not_found() {
    let entry = model(&[("first", "sub/first.bin")], &[("second", "2")]);
    for name in ["First", "firs", "first ", "second", ""] {
        assert_eq!(entry.named_path(name), None, "{name:?}");
    }
    assert_eq!(model(&[], &[]).named_path("first"), None);
}

#[test]
fn paths_and_settings_with_the_same_name_do_not_get_mixed_up() {
    let entry = model(&[("shared", "sub/file.bin")], &[("shared", "7")]);
    entry.validate().expect("valid");
    assert_eq!(entry.named_path("shared"), Some("sub/file.bin"));
    assert_eq!(entry.engine_setting("shared"), Some("7"));
}

#[test]
fn the_same_engine_setting_twice_is_refused() {
    assert_refused(
        "a setting twice with different values",
        &model(&[], &[("rate", "8000"), ("rate", "16000")]),
    );
    assert_refused(
        "a setting twice with the same value",
        &model(
            &[],
            &[("kind", "kind-one"), ("rate", "1"), ("kind", "kind-one")],
        ),
    );
}

#[test]
fn a_blank_engine_setting_name_is_refused() {
    for blank in ["", " ", "   ", "\t"] {
        assert_refused(
            &format!("the setting name {blank:?}"),
            &model(&[], &[("rate", "8000"), (blank, "1")]),
        );
    }
}

#[test]
fn an_engine_setting_name_with_spaces_around_it_is_refused() {
    for name in [" rate", "rate ", "\trate\n"] {
        assert_refused(
            &format!("the setting name {name:?}"),
            &model(&[], &[(name, "8000")]),
        );
    }
}

#[test]
fn the_same_named_path_twice_or_a_blank_path_name_is_refused() {
    assert_refused(
        "a path name twice",
        &model(&[("first", "a.bin"), ("first", "b.bin")], &[]),
    );
    for blank in ["", "  "] {
        assert_refused(
            &format!("the path name {blank:?}"),
            &model(&[(blank, "a.bin")], &[]),
        );
    }
}

#[test]
fn settings_that_differ_in_name_are_all_accepted() {
    let entry = model(
        &[("first", "a.bin"), ("First", "b.bin")],
        &[("rate", "1"), ("Rate", "2"), ("rate2", "3")],
    );
    entry.validate().expect("valid");
    assert_eq!(entry.engine_setting("Rate"), Some("2"));
    assert_eq!(entry.named_path("First"), Some("b.bin"));
}
