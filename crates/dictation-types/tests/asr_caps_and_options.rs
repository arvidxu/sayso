mod support;

use dictation_types::{
    AsrCaps, AsrOptions, ErrorKind, LanguageMixing, ProviderRestriction, Term, TermId, TermKind,
};
use support::{engine, lang, langs, mixing_up_to, one_per_utterance, tags};

fn hotword(text: &str, tag: &str) -> Term {
    Term::new(TermId::new(text), text, TermKind::Word, lang(tag)).unwrap()
}

#[test]
fn a_conservative_declaration_is_valid() {
    assert!(one_per_utterance(&["sw"]).validate().is_ok());
    assert!(
        one_per_utterance(&["pt", "it", "bn", "en", "ru", "ko", "fr"])
            .validate()
            .is_ok()
    );
}

#[test]
fn an_engine_that_can_neither_stream_nor_batch_is_invalid() {
    let mut caps = one_per_utterance(&["ko"]);
    caps.supports_streaming = false;
    caps.supports_batch = false;
    assert_eq!(caps.validate().unwrap_err().kind, ErrorKind::Validation);
}

#[test]
fn an_engine_without_languages_is_invalid() {
    let caps = AsrCaps::new(Vec::new(), 16_000, true);
    assert_eq!(caps.validate().unwrap_err().kind, ErrorKind::Validation);
}

#[test]
fn an_engine_with_a_zero_sample_rate_is_invalid() {
    let caps = AsrCaps::new(tags(&["ko"]), 0, true);
    assert!(caps.validate().is_err());
}

#[test]
fn an_engine_with_a_zero_duration_limit_is_invalid() {
    let mut caps = one_per_utterance(&["ko"]);
    caps.max_duration_secs = Some(0);
    assert!(caps.validate().is_err());
    caps.max_duration_secs = Some(1);
    assert!(caps.validate().is_ok());
}

#[test]
fn a_mixing_limit_below_two_is_invalid() {
    assert!(mixing_up_to(&["ru", "ko"], Some(0)).validate().is_err());
    assert!(mixing_up_to(&["ru", "ko"], Some(1)).validate().is_err());
    assert!(mixing_up_to(&["ru", "ko"], Some(2)).validate().is_ok());
    assert!(mixing_up_to(&["ru", "ko"], None).validate().is_ok());
}

#[test]
fn a_mixing_group_naming_an_unsupported_language_is_invalid() {
    let caps = engine(
        &["ru", "ko"],
        LanguageMixing::MixedWithinGroups(vec![tags(&["ru", "ko"]), tags(&["ru", "fr"])]),
    );
    assert!(caps.validate().is_err());

    let fine = engine(
        &["ru", "ko", "fr-FR"],
        LanguageMixing::MixedWithinGroups(vec![tags(&["ru", "ko"]), tags(&["ru", "fr-CA"])]),
    );
    assert!(fine.validate().is_ok());
}

#[test]
fn plain_options_ask_for_nothing_optional() {
    let opts = AsrOptions::new(langs("bn+en"));
    assert_eq!(opts.languages, langs("bn+en"));
    assert_eq!(opts.model_id, None, "no model may be chosen for the user");
    assert!(!opts.streaming);
    assert!(opts.hotwords.is_empty());
    assert!(!opts.per_segment_language);
}

#[test]
fn plain_options_are_acceptable_to_any_valid_batch_engine_for_that_language() {
    let supported = ["pt-BR", "bn", "ko", "sw", "el"];
    let mut engines = vec![
        one_per_utterance(&supported),
        engine(&supported, LanguageMixing::AutoDetectOne),
        mixing_up_to(&supported, None),
        mixing_up_to(&supported, Some(2)),
        engine(
            &supported,
            LanguageMixing::MixedWithinGroups(vec![tags(&["pt-BR", "sw"]), tags(&["bn", "ko"])]),
        ),
    ];
    for mut caps in engines.clone() {
        caps.supports_streaming = true;
        caps.supports_hotwords = true;
        caps.supports_per_segment_language = true;
        caps.max_duration_secs = Some(30);
        caps.operates_locally = true;
        caps.sample_rate_hz = 48_000;
        engines.push(caps);
    }

    for caps in &engines {
        assert!(caps.validate().is_ok(), "{caps:?}");
        assert!(caps.supports_batch);
        for tag in ["pt", "pt-PT", "bn-IN", "ko", "sw-KE", "el"] {
            let opts = AsrOptions::new(langs(tag));
            assert_eq!(
                caps.check_options(&opts, None),
                [],
                "{tag} on {:?}",
                caps.language_mixing
            );
        }
    }
}

#[test]
fn asking_for_per_segment_languages_from_an_engine_that_cannot_give_them_is_reported() {
    let caps = mixing_up_to(&["ru", "ko", "fr"], None);
    assert!(!caps.supports_per_segment_language);
    let mut opts = AsrOptions::new(langs("ru+ko+fr"));
    opts.per_segment_language = true;
    assert_eq!(
        caps.check_options(&opts, None),
        [ProviderRestriction::PerSegmentLanguageUnsupported]
    );

    let mut able = caps.clone();
    able.supports_per_segment_language = true;
    assert!(able.check_options(&opts, None).is_empty());
}

#[test]
fn asking_for_streaming_from_a_batch_only_engine_is_reported() {
    let caps = one_per_utterance(&["ta"]);
    let mut opts = AsrOptions::new(langs("ta"));
    opts.streaming = true;
    assert_eq!(
        caps.check_options(&opts, None),
        [ProviderRestriction::StreamingUnsupported]
    );
}

#[test]
fn asking_for_batch_from_a_streaming_only_engine_is_reported() {
    let mut caps = one_per_utterance(&["ta"]);
    caps.supports_streaming = true;
    caps.supports_batch = false;
    assert!(caps.validate().is_ok());
    assert_eq!(
        caps.check_options(&AsrOptions::new(langs("ta")), None),
        [ProviderRestriction::BatchUnsupported]
    );
}

#[test]
fn hotwords_for_an_engine_that_takes_none_are_reported() {
    let caps = one_per_utterance(&["el"]);
    let mut opts = AsrOptions::new(langs("el"));
    opts.hotwords = vec![hotword("Θεσσαλονίκη", "el")];
    assert_eq!(
        caps.check_options(&opts, None),
        [ProviderRestriction::HotwordsUnsupported]
    );

    let mut able = caps.clone();
    able.supports_hotwords = true;
    assert!(able.check_options(&opts, None).is_empty());
}

#[test]
fn a_recording_longer_than_the_limit_is_reported_with_the_limit() {
    let mut caps = one_per_utterance(&["he"]);
    caps.max_duration_secs = Some(30);
    let opts = AsrOptions::new(langs("he"));

    assert!(caps.check_options(&opts, Some(29)).is_empty());
    assert!(caps.check_options(&opts, Some(30)).is_empty());
    assert_eq!(
        caps.check_options(&opts, Some(31)),
        [ProviderRestriction::MaxDurationSecs(30)]
    );
    assert!(caps.check_options(&opts, None).is_empty());

    assert!(!caps.exceeds_max_duration(30));
    assert!(caps.exceeds_max_duration(31));
}

#[test]
fn an_engine_without_a_duration_limit_takes_any_length() {
    let caps = one_per_utterance(&["he"]);
    assert!(!caps.exceeds_max_duration(u32::MAX));
    assert!(
        caps.check_options(&AsrOptions::new(langs("he")), Some(u32::MAX))
            .is_empty()
    );
}

#[test]
fn every_problem_with_a_request_is_reported_together() {
    let mut caps = one_per_utterance(&["ru", "ko"]);
    caps.max_duration_secs = Some(10);
    let mut opts = AsrOptions::new(langs("ru+ko"));
    opts.streaming = true;
    opts.per_segment_language = true;
    opts.hotwords = vec![hotword("Владивосток", "ru")];

    let report = caps.check_options(&opts, Some(60));
    for expected in [
        ProviderRestriction::StreamingUnsupported,
        ProviderRestriction::MaxDurationSecs(10),
        ProviderRestriction::HotwordsUnsupported,
        ProviderRestriction::PerSegmentLanguageUnsupported,
        ProviderRestriction::LanguageMixingUnsupported,
    ] {
        assert!(
            report.contains(&expected),
            "{expected:?} missing from {report:?}"
        );
    }
    assert_eq!(report.len(), 5);
}

#[test]
fn language_problems_are_part_of_the_options_report() {
    let caps = one_per_utterance(&["pt", "it"]);
    assert_eq!(
        caps.check_options(&AsrOptions::new(langs("bn+en")), None),
        [ProviderRestriction::UnsupportedLanguages(tags(&[
            "bn", "en"
        ]))]
    );
    assert_eq!(
        caps.check_options(&AsrOptions::new(langs("pt+it")), None),
        [ProviderRestriction::LanguageMixingUnsupported]
    );
}

#[test]
fn the_declared_limits_list_everything_the_engine_cannot_do() {
    let mut caps = one_per_utterance(&["am"]);
    caps.max_duration_secs = Some(45);
    let limits = caps.limits();
    for expected in [
        ProviderRestriction::StreamingUnsupported,
        ProviderRestriction::MaxDurationSecs(45),
        ProviderRestriction::HotwordsUnsupported,
        ProviderRestriction::LanguageMixingUnsupported,
        ProviderRestriction::PerSegmentLanguageUnsupported,
    ] {
        assert!(
            limits.contains(&expected),
            "{expected:?} missing from {limits:?}"
        );
    }
    assert!(!limits.contains(&ProviderRestriction::BatchUnsupported));
}

#[test]
fn an_engine_that_can_do_everything_declares_no_limits() {
    let mut caps = mixing_up_to(&["am", "ti", "so"], None);
    caps.supports_streaming = true;
    caps.supports_hotwords = true;
    caps.supports_per_segment_language = true;
    assert_eq!(caps.limits(), []);
}
