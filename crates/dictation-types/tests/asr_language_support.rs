mod support;

use dictation_types::{LanguageMixing, ProviderRestriction};
use support::{engine, lang, langs, mixing_up_to, one_per_utterance, tags};

#[test]
fn a_language_is_supported_in_all_its_regional_variants() {
    let caps = one_per_utterance(&["pt-BR", "bn", "zh-Hans"]);
    for tag in [
        "pt",
        "pt-BR",
        "PT_br",
        "pt-PT",
        "bn-IN",
        "bn-BD",
        "zh",
        "zh-Hant-HK",
        "zh-TW",
    ] {
        assert!(
            caps.supports_language(&lang(tag)),
            "{tag} must be supported"
        );
        assert!(
            caps.supports_languages(&langs(tag)),
            "{tag} must be supported"
        );
        assert!(caps.check_languages(&langs(tag)).is_empty(), "{tag}");
    }
}

#[test]
fn a_language_the_engine_does_not_list_is_not_supported() {
    let caps = one_per_utterance(&["pt-BR", "bn"]);
    for tag in ["it", "es-BR", "hi-IN", "yue", "en"] {
        assert!(!caps.supports_language(&lang(tag)), "{tag}");
        assert!(!caps.supports_languages(&langs(tag)), "{tag}");
    }
}

#[test]
fn unsupported_languages_are_reported_by_name() {
    let caps = mixing_up_to(&["pt", "it"], None);
    assert_eq!(
        caps.check_languages(&langs("bn+en")),
        [ProviderRestriction::UnsupportedLanguages(tags(&[
            "bn", "en"
        ]))]
    );
}

#[test]
fn only_the_unsupported_languages_of_a_request_are_named() {
    let caps = mixing_up_to(&["pt", "it", "ru"], None);
    assert_eq!(
        caps.check_languages(&langs("pt+ko+ru+fr")),
        [ProviderRestriction::UnsupportedLanguages(tags(&[
            "ko", "fr"
        ]))]
    );
    assert_eq!(
        caps.check_languages(&langs("ta")),
        [ProviderRestriction::UnsupportedLanguages(tags(&["ta"]))]
    );
}

#[test]
fn the_unsupported_language_is_reported_as_the_user_wrote_it() {
    let caps = one_per_utterance(&["ru"]);
    assert_eq!(
        caps.check_languages(&langs("zh-Hant-HK")),
        [ProviderRestriction::UnsupportedLanguages(tags(&[
            "zh-Hant-HK"
        ]))]
    );
}

#[test]
fn an_unsupported_language_is_reported_whatever_the_mixing_ability() {
    let supported = ["pt", "it"];
    let engines = [
        one_per_utterance(&supported),
        engine(&supported, LanguageMixing::AutoDetectOne),
        mixing_up_to(&supported, None),
        mixing_up_to(&supported, Some(2)),
        engine(
            &supported,
            LanguageMixing::MixedWithinGroups(vec![tags(&supported)]),
        ),
    ];
    for caps in engines {
        let report = caps.check_languages(&langs("pt+sw"));
        assert_eq!(
            report,
            [ProviderRestriction::UnsupportedLanguages(tags(&["sw"]))],
            "{:?}",
            caps.language_mixing
        );
        assert!(!caps.supports_languages(&langs("pt+sw")));
    }
}

#[test]
fn an_engine_limited_to_one_language_per_utterance_reports_that_for_two_languages() {
    let caps = one_per_utterance(&["pt", "it", "bn", "en"]);
    for spec in ["pt+it", "bn+en", "en+bn", "pt+it+bn+en"] {
        assert_eq!(
            caps.check_languages(&langs(spec)),
            [ProviderRestriction::LanguageMixingUnsupported],
            "{spec}"
        );
        assert!(!caps.supports_languages(&langs(spec)));
    }
}

#[test]
fn an_engine_limited_to_one_language_per_utterance_accepts_any_single_supported_language() {
    let caps = one_per_utterance(&["pt", "it", "bn", "en"]);
    for spec in ["pt", "it", "bn", "en", "pt-BR", "bn-IN"] {
        assert!(caps.check_languages(&langs(spec)).is_empty(), "{spec}");
    }
}

#[test]
fn regional_variants_of_one_language_are_not_a_mixing_request() {
    let caps = one_per_utterance(&["pt", "fr", "zh"]);
    for spec in ["pt-BR+pt-PT", "fr-FR+fr-CA+fr-BE", "zh-CN+zh-TW+zh-Hant-HK"] {
        assert!(
            caps.check_languages(&langs(spec)).is_empty(),
            "{spec} is one language and needs no mixing"
        );
    }
}

#[test]
fn an_auto_detecting_engine_accepts_several_languages() {
    let caps = engine(
        &["ru", "ko", "fr", "pt", "it"],
        LanguageMixing::AutoDetectOne,
    );
    for spec in ["ru+ko", "ru+ko+fr", "pt+it+ru+ko+fr"] {
        assert!(caps.supports_languages(&langs(spec)), "{spec}");
    }
}

#[test]
fn an_auto_detecting_engine_is_told_apart_from_one_that_has_to_be_told_the_language() {
    let supported = ["ru", "ko", "fr"];
    let told = one_per_utterance(&supported);
    let auto = engine(&supported, LanguageMixing::AutoDetectOne);
    let request = langs("ru+ko+fr");
    assert_ne!(
        (told.check_languages(&request), told.limits()),
        (auto.check_languages(&request), auto.limits())
    );
}

#[test]
fn an_auto_detecting_engine_is_told_apart_from_one_that_mixes_freely() {
    // Auto-detecting one language per utterance is not following a mid-sentence switch;
    // the report must show the difference.
    let supported = ["ru", "ko", "fr"];
    let auto = engine(&supported, LanguageMixing::AutoDetectOne);
    let mixed = mixing_up_to(&supported, None);
    let request = langs("ru+ko+fr");
    assert_ne!(
        (auto.check_languages(&request), auto.limits()),
        (mixed.check_languages(&request), mixed.limits()),
        "the reports of an auto-detecting engine and a freely mixing engine are identical"
    );
}

#[test]
fn an_engine_that_mixes_only_within_groups_is_told_apart_from_one_that_mixes_freely() {
    let supported = ["pt", "es", "ru", "uk"];
    let grouped = engine(
        &supported,
        LanguageMixing::MixedWithinGroups(vec![tags(&["pt", "es"]), tags(&["ru", "uk"])]),
    );
    let mixed = mixing_up_to(&supported, None);
    assert_ne!(
        grouped.limits(),
        mixed.limits(),
        "the declared limits of a group-limited engine and a freely mixing engine are identical"
    );
}

#[test]
fn a_mixing_engine_accepts_any_number_of_languages_when_it_declares_no_limit() {
    let all = ["pt", "it", "bn", "en", "ru", "ko", "fr", "ta", "ar", "sw"];
    let caps = mixing_up_to(&all, None);
    for spec in [
        "pt+it",
        "bn+en",
        "ru+ko+fr",
        "pt+it+bn+en",
        "pt+it+bn+en+ru+ko+fr+ta+ar+sw",
    ] {
        assert!(caps.check_languages(&langs(spec)).is_empty(), "{spec}");
    }
}

#[test]
fn a_mixing_engine_reports_its_limit_when_more_languages_are_requested() {
    let caps = mixing_up_to(&["pt", "it", "bn", "en", "ru"], Some(2));
    assert!(caps.check_languages(&langs("pt+it")).is_empty());
    assert!(caps.check_languages(&langs("bn+en")).is_empty());
    assert_eq!(
        caps.check_languages(&langs("ru+bn+en")),
        [ProviderRestriction::TooManyLanguages { max: 2 }]
    );

    let three = mixing_up_to(&["pt", "it", "bn", "en", "ru"], Some(3));
    assert!(three.check_languages(&langs("ru+bn+en")).is_empty());
    assert_eq!(
        three.check_languages(&langs("ru+bn+en+pt")),
        [ProviderRestriction::TooManyLanguages { max: 3 }]
    );
}

#[test]
fn regional_variants_count_once_towards_the_mixing_limit() {
    let caps = mixing_up_to(&["pt", "it", "fr"], Some(2));
    assert!(caps.check_languages(&langs("pt-BR+pt-PT+it")).is_empty());
    assert!(
        caps.check_languages(&langs("fr-FR+fr-CA+pt-BR+pt-PT"))
            .is_empty()
    );
}

#[test]
fn group_mixing_accepts_languages_of_one_group() {
    let caps = engine(
        &["pt", "es", "it", "ru", "uk", "kk"],
        LanguageMixing::MixedWithinGroups(vec![
            tags(&["pt", "es", "it"]),
            tags(&["ru", "uk", "kk"]),
        ]),
    );
    for spec in [
        "pt+es",
        "es+it+pt",
        "ru+kk",
        "uk+ru+kk",
        "pt-BR+es-MX",
        "it",
    ] {
        assert!(caps.check_languages(&langs(spec)).is_empty(), "{spec}");
    }
}

#[test]
fn group_mixing_reports_a_combination_that_spans_groups() {
    let caps = engine(
        &["pt", "es", "it", "ru", "uk", "kk"],
        LanguageMixing::MixedWithinGroups(vec![
            tags(&["pt", "es", "it"]),
            tags(&["ru", "uk", "kk"]),
        ]),
    );
    for spec in ["pt+ru", "uk+it", "pt+es+kk"] {
        assert_eq!(
            caps.check_languages(&langs(spec)),
            [ProviderRestriction::LanguageCombinationUnsupported],
            "{spec}"
        );
    }
}

#[test]
fn a_single_supported_language_outside_every_group_is_still_accepted() {
    let caps = engine(
        &["pt", "es", "ta"],
        LanguageMixing::MixedWithinGroups(vec![tags(&["pt", "es"])]),
    );
    assert!(caps.check_languages(&langs("ta")).is_empty());
    assert!(caps.check_languages(&langs("ta-IN+ta-LK")).is_empty());
}

#[test]
fn each_kind_of_refusal_is_a_different_report() {
    let request = langs("ru+ko+fr");
    let unsupported = mixing_up_to(&["ru", "ko"], None).check_languages(&request);
    let one_only = one_per_utterance(&["ru", "ko", "fr"]).check_languages(&request);
    let too_many = mixing_up_to(&["ru", "ko", "fr"], Some(2)).check_languages(&request);
    let wrong_group = engine(
        &["ru", "ko", "fr"],
        LanguageMixing::MixedWithinGroups(vec![tags(&["ru", "ko"]), tags(&["fr"])]),
    )
    .check_languages(&request);

    let reports = [unsupported, one_only, too_many, wrong_group];
    for (i, a) in reports.iter().enumerate() {
        assert_eq!(a.len(), 1, "one reason each: {a:?}");
        for b in &reports[i + 1..] {
            assert_ne!(a, b);
        }
    }
}

#[test]
fn the_declared_limits_tell_the_mixing_abilities_apart() {
    let supported = ["ru", "ko", "fr"];
    let one = one_per_utterance(&supported).limits();
    let two = mixing_up_to(&supported, Some(2)).limits();
    let three = mixing_up_to(&supported, Some(3)).limits();
    let unlimited = mixing_up_to(&supported, None).limits();

    assert!(one.contains(&ProviderRestriction::LanguageMixingUnsupported));
    assert!(two.contains(&ProviderRestriction::TooManyLanguages { max: 2 }));
    assert!(three.contains(&ProviderRestriction::TooManyLanguages { max: 3 }));
    assert!(!unlimited.contains(&ProviderRestriction::LanguageMixingUnsupported));
    assert!(
        !unlimited
            .iter()
            .any(|r| matches!(r, ProviderRestriction::TooManyLanguages { .. }))
    );
    assert_ne!(one, two);
    assert_ne!(two, three);
    assert_ne!(three, unlimited);
}
