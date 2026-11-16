mod support;

use asr_local::capabilities_of;
use dictation_types::{ErrorKind, LanguageMixing, ModelDescriptor, ModelKind};
use support::{ENGINE, entry_of_kind, language, speech_entry, with_setting};

fn entry(settings: &[(&str, &str)]) -> ModelDescriptor {
    speech_entry("model-a", ENGINE, &["sw", "ko", "fi", "ta"], settings)
}

fn complete_entry() -> ModelDescriptor {
    entry(&[("sampleRateHz", "16000"), ("languageMixing", "one")])
}

fn assert_refused(what: &str, model: &ModelDescriptor) {
    match capabilities_of(model) {
        Ok(caps) => panic!("an entry with {what} was accepted: {caps:?}"),
        Err(err) => assert_eq!(err.kind, ErrorKind::Validation, "{what}: {err}"),
    }
}

#[test]
fn the_sample_rate_comes_from_the_entry() {
    for rate in [8_000u32, 16_000, 22_050, 48_000] {
        let model = with_setting(complete_entry(), "sampleRateHz", Some(&rate.to_string()));
        assert_eq!(capabilities_of(&model).unwrap().sample_rate_hz, rate);
    }
}

#[test]
fn the_longest_audio_comes_from_the_entry_and_may_be_left_out() {
    assert_eq!(
        capabilities_of(&complete_entry())
            .unwrap()
            .max_duration_secs,
        None
    );
    for secs in [1u32, 30, 3_600] {
        let model = with_setting(complete_entry(), "maxAudioSecs", Some(&secs.to_string()));
        assert_eq!(
            capabilities_of(&model).unwrap().max_duration_secs,
            Some(secs)
        );
    }
}

#[test]
fn every_kind_of_language_mixing_can_be_declared() {
    let cases = [
        ("one", LanguageMixing::OnePerUtterance),
        ("auto-detect-one", LanguageMixing::AutoDetectOne),
        (
            "mixed",
            LanguageMixing::Mixed {
                max_languages: None,
            },
        ),
    ];
    for (text, expected) in cases {
        let model = with_setting(complete_entry(), "languageMixing", Some(text));
        assert_eq!(
            capabilities_of(&model).unwrap().language_mixing,
            expected,
            "{text}"
        );
    }
}

#[test]
fn a_mixing_model_may_limit_how_many_languages_it_mixes() {
    for max in [2usize, 3, 17] {
        let model = with_setting(
            with_setting(complete_entry(), "languageMixing", Some("mixed")),
            "maxMixedLanguages",
            Some(&max.to_string()),
        );
        assert_eq!(
            capabilities_of(&model).unwrap().language_mixing,
            LanguageMixing::Mixed {
                max_languages: Some(max)
            }
        );
    }
}

#[test]
fn the_supported_languages_are_those_the_entry_lists() {
    for languages in [
        vec!["sw"],
        vec!["ko", "fi", "ta"],
        vec!["pt-BR", "ar", "hi", "vi", "pl", "el"],
    ] {
        let model = speech_entry(
            "model-a",
            ENGINE,
            &languages,
            &[("sampleRateHz", "16000"), ("languageMixing", "mixed")],
        );
        let expected: Vec<_> = languages.iter().map(|tag| language(tag)).collect();
        assert_eq!(
            capabilities_of(&model).unwrap().supported_languages,
            expected
        );
    }
}

#[test]
fn a_model_on_this_device_is_local_batch_only_and_takes_no_hotwords() {
    for mixing in ["one", "auto-detect-one", "mixed"] {
        let model = with_setting(complete_entry(), "languageMixing", Some(mixing));
        let caps = capabilities_of(&model).unwrap();
        assert!(caps.operates_locally);
        assert!(caps.supports_batch);
        assert!(!caps.supports_streaming);
        assert!(!caps.supports_hotwords);
        caps.validate().expect("consistent capabilities");
    }
}

#[test]
fn settings_meant_for_the_engine_alone_do_not_get_in_the_way() {
    let model = with_setting(complete_entry(), "somethingElse", Some("whatever"));
    assert_eq!(
        capabilities_of(&model).unwrap(),
        capabilities_of(&complete_entry()).unwrap()
    );
}

#[test]
fn a_model_that_does_not_recognise_speech_is_refused() {
    for kind in [ModelKind::Llm, ModelKind::Vad] {
        let model = entry_of_kind(
            kind,
            "model-b",
            ENGINE,
            &["sw", "ko"],
            &[("sampleRateHz", "16000"), ("languageMixing", "one")],
        );
        assert_refused(&format!("the kind {kind:?}"), &model);
    }
}

#[test]
fn a_missing_sample_rate_is_refused() {
    assert_refused(
        "no sample rate",
        &with_setting(complete_entry(), "sampleRateHz", None),
    );
}

#[test]
fn a_sample_rate_that_is_not_a_whole_positive_number_is_refused() {
    for bad in ["", "fast", "16k", "16000.5", "-16000", "1e4", "0", "0x10"] {
        assert_refused(
            &format!("the sample rate '{bad}'"),
            &with_setting(complete_entry(), "sampleRateHz", Some(bad)),
        );
    }
}

#[test]
fn missing_language_mixing_is_refused() {
    assert_refused(
        "no language mixing",
        &with_setting(complete_entry(), "languageMixing", None),
    );
}

#[test]
fn unknown_language_mixing_is_refused() {
    for bad in ["", "two", "any", "ONE", "auto", "mixed-up", "true"] {
        assert_refused(
            &format!("the language mixing '{bad}'"),
            &with_setting(complete_entry(), "languageMixing", Some(bad)),
        );
    }
}

#[test]
fn a_longest_audio_that_is_not_a_whole_positive_number_is_refused() {
    for bad in ["", "long", "30s", "2.5", "-1", "0"] {
        assert_refused(
            &format!("the longest audio '{bad}'"),
            &with_setting(complete_entry(), "maxAudioSecs", Some(bad)),
        );
    }
}

#[test]
fn a_mixing_limit_that_is_not_a_whole_number_of_at_least_two_is_refused() {
    let mixed = with_setting(complete_entry(), "languageMixing", Some("mixed"));
    for bad in ["", "many", "2.5", "-3", "0", "1"] {
        assert_refused(
            &format!("the mixing limit '{bad}'"),
            &with_setting(mixed.clone(), "maxMixedLanguages", Some(bad)),
        );
    }
}

#[test]
fn a_model_takes_hotwords_only_when_its_entry_says_so() {
    assert!(
        !capabilities_of(&complete_entry())
            .unwrap()
            .supports_hotwords
    );
    for (text, takes) in [("true", true), ("false", false), (" true ", true)] {
        let model = with_setting(complete_entry(), "supportsHotwords", Some(text));
        assert_eq!(
            capabilities_of(&model).unwrap().supports_hotwords,
            takes,
            "{text}"
        );
    }
    for text in ["yes", "1", "", "TRUE"] {
        let model = with_setting(complete_entry(), "supportsHotwords", Some(text));
        let err = capabilities_of(&model).expect_err(text);
        assert_eq!(err.kind, ErrorKind::Validation, "{text}: {err}");
        assert!(err.message.contains("supportsHotwords"), "{err}");
    }
}
