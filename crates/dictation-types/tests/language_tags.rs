mod support;

use std::collections::HashSet;
use std::str::FromStr;

use dictation_types::{ErrorKind, Language};
use support::lang;

#[test]
fn the_same_tag_in_any_letter_case_is_the_same_tag() {
    for (a, b) in [
        ("pt-BR", "PT-br"),
        ("pt-BR", "Pt-Br"),
        ("bn", "BN"),
        ("zh-Hant-HK", "ZH-HANT-hk"),
        ("sr-Latn-RS", "SR-latn-rs"),
        ("yue", "YUE"),
        ("es-419", "ES-419"),
    ] {
        assert_eq!(lang(a), lang(b), "{a} and {b} must be the same tag");
    }
}

#[test]
fn underscore_and_hyphen_separators_give_the_same_tag() {
    for (a, b) in [
        ("pt-BR", "pt_BR"),
        ("pt-BR", "PT_br"),
        ("zh-Hant-HK", "zh_Hant_HK"),
        ("zh-Hant-HK", "zh_hant-hk"),
        ("ko-KR", "ko_kr"),
        ("uz-Cyrl", "UZ_CYRL"),
    ] {
        assert_eq!(lang(a), lang(b), "{a} and {b} must be the same tag");
    }
}

#[test]
fn equal_tags_hash_to_one_entry() {
    let set: HashSet<Language> = ["pt-BR", "PT_br", "pt_BR", "Pt-bR"]
        .into_iter()
        .map(lang)
        .collect();
    assert_eq!(set.len(), 1);
}

#[test]
fn a_tag_is_written_back_in_canonical_form() {
    for (raw, canonical) in [
        ("PT_br", "pt-BR"),
        ("ZH_hant_hk", "zh-Hant-HK"),
        ("SR_LATN_rs", "sr-Latn-RS"),
        ("IT", "it"),
        ("es_419", "es-419"),
        ("YUE-hant-HK", "yue-Hant-HK"),
    ] {
        assert_eq!(lang(raw).code(), canonical);
        assert_eq!(lang(raw).to_string(), canonical);
    }
}

#[test]
fn zh_hant_hk_survives_a_round_trip() {
    let tag = lang("zh-Hant-HK");
    assert_eq!(tag.code(), "zh-Hant-HK");
    assert_eq!(tag.primary(), "zh");
    assert_eq!(tag.script_subtag(), Some("Hant"));
    assert_eq!(tag.region(), Some("HK"));
    assert_eq!(lang(&tag.code()), tag);
    assert_eq!(Language::from_str(&tag.to_string()).unwrap(), tag);
    assert_eq!("zh-Hant-HK".parse::<Language>().unwrap(), tag);
}

#[test]
fn every_valid_tag_survives_a_round_trip_through_its_text_form() {
    for raw in [
        "pt",
        "pt-BR",
        "PT_br",
        "pt-PT",
        "bn-IN",
        "ru",
        "ko-KR",
        "fr-CA",
        "zh-Hans-CN",
        "zh-Hant-HK",
        "sr-Latn-RS",
        "uz-Cyrl-UZ",
        "es-419",
        "yue",
        "fil-PH",
        "haw",
        "de-CH-1901",
        "ca-ES-valencia",
        "sl-rozaj-biske",
        "en-x-private",
        "en-US-u-nu-thai",
    ] {
        let first = lang(raw);
        let second = lang(&first.code());
        assert_eq!(first, second, "'{raw}' changed after a round trip");
        assert_eq!(first.code(), second.code());
    }
}

#[test]
fn a_language_matches_its_regional_variants() {
    let pt = lang("pt");
    let brazil = lang("pt-BR");
    let brazil_sloppy = lang("PT_br");
    let portugal = lang("pt-PT");

    assert_eq!(brazil, brazil_sloppy);
    for a in [&pt, &brazil, &brazil_sloppy, &portugal] {
        for b in [&pt, &brazil, &brazil_sloppy, &portugal] {
            assert!(a.matches(b), "{a} must match {b}");
        }
    }
}

#[test]
fn regional_variants_are_the_same_language_but_not_the_same_tag() {
    assert_ne!(lang("pt-BR"), lang("pt-PT"));
    assert_ne!(lang("pt"), lang("pt-BR"));
    assert_ne!(lang("fr-CA"), lang("fr-FR"));
    assert_ne!(lang("zh-Hans"), lang("zh-Hant"));
}

#[test]
fn a_language_matches_its_script_variants() {
    assert!(lang("zh-Hans").matches(&lang("zh-Hant-HK")));
    assert!(lang("sr-Latn").matches(&lang("sr-Cyrl-RS")));
    assert!(lang("uz").matches(&lang("uz-Cyrl")));
    assert!(lang("pa-Arab").matches(&lang("pa-Guru-IN")));
}

#[test]
fn different_languages_do_not_match() {
    for (a, b) in [
        ("pt", "it"),
        ("pt-BR", "es-BR"),
        ("bn-IN", "hi-IN"),
        ("ru", "uk"),
        ("zh", "yue"),
        ("ko", "ja"),
        ("sr-Latn", "hr-Latn"),
    ] {
        assert!(!lang(a).matches(&lang(b)), "{a} must not match {b}");
        assert!(!lang(b).matches(&lang(a)), "{b} must not match {a}");
    }
}

#[test]
fn three_letter_language_codes_are_tags() {
    for (raw, primary) in [
        ("yue", "yue"),
        ("fil", "fil"),
        ("haw", "haw"),
        ("ceb", "ceb"),
        ("CMN", "cmn"),
        ("yue-HK", "yue"),
        ("fil_PH", "fil"),
    ] {
        assert_eq!(lang(raw).primary(), primary);
    }
    assert_eq!(lang("yue-Hant-HK").script_subtag(), Some("Hant"));
    assert_eq!(lang("yue-Hant-HK").region(), Some("HK"));
    assert_eq!(lang("fil_ph").region(), Some("PH"));
}

#[test]
fn a_numeric_region_is_a_region() {
    let latin_america = lang("es-419");
    assert_eq!(latin_america.region(), Some("419"));
    assert_eq!(latin_america.script_subtag(), None);
    assert!(latin_america.matches(&lang("es-MX")));
}

#[test]
fn a_bare_language_has_no_script_or_region() {
    let tag = lang("ko");
    assert_eq!(tag.primary(), "ko");
    assert_eq!(tag.script_subtag(), None);
    assert_eq!(tag.region(), None);
}

#[test]
fn tags_with_variant_or_private_subtags_are_still_their_language() {
    for (raw, base) in [
        ("en-x-private", "en"),
        ("de-CH-1901", "de"),
        ("ca-ES-valencia", "ca"),
        ("sl-rozaj-biske", "sl"),
        ("sr-Latn-RS-x-custom", "sr"),
        ("th-TH-u-nu-thai", "th"),
    ] {
        let tag = lang(raw);
        assert_eq!(tag.primary(), base);
        assert!(tag.matches(&lang(base)), "{raw} must match {base}");
        assert!(lang(base).matches(&tag), "{base} must match {raw}");
    }
}

#[test]
fn tags_with_extra_subtags_keep_their_script_and_region() {
    let swiss = lang("de-CH-1901");
    assert_eq!(swiss.region(), Some("CH"));

    let serbian = lang("sr-Latn-RS-x-custom");
    assert_eq!(serbian.script_subtag(), Some("Latn"));
    assert_eq!(serbian.region(), Some("RS"));
}

#[test]
fn tags_that_differ_only_in_extra_subtags_are_different_tags() {
    assert_ne!(lang("de-CH-1901"), lang("de-CH-1996"));
    assert_ne!(lang("de-CH"), lang("de-CH-1901"));
    assert_eq!(lang("de-CH-1901"), lang("DE_ch_1901"));
    assert_eq!(lang("ca-ES-valencia"), lang("CA_es_VALENCIA"));
}

#[test]
fn an_extension_subtag_is_not_mistaken_for_a_script() {
    // `u-nu-thai` asks for Thai digits; the tag is still English written in Latin letters.
    let tag = lang("en-US-u-nu-thai");
    assert_eq!(tag.region(), Some("US"));
    assert_eq!(tag.script_subtag(), None);
}

#[test]
fn an_extended_language_subtag_does_not_hide_the_script_and_region() {
    // `zh-cmn-Hant-TW` is the long form of Mandarin in traditional characters for Taiwan.
    let tag = lang("zh-cmn-Hant-TW");
    assert_eq!(tag.primary(), "zh");
    assert!(tag.matches(&lang("zh")));
    assert_eq!(tag.script_subtag(), Some("Hant"));
    assert_eq!(tag.region(), Some("TW"));
}

#[test]
fn clearly_invalid_input_is_not_a_tag() {
    let invalid = [
        "",
        " ",
        "-",
        "_",
        "e",
        "1",
        "123",
        "1en",
        "12-US",
        "e1",
        "en--US",
        "en__US",
        "en-",
        "en_",
        "-en",
        "en-US-",
        "en US",
        "en-U$",
        "en.US",
        "en-U S",
        "fr!",
        "pt@BR",
        "en-ÜS",
        "ру",
        "한국어",
        "বাংলা",
        "abcdefghi",
    ];
    let accepted: Vec<&str> = invalid
        .into_iter()
        .filter(|raw| Language::parse(raw).is_ok())
        .collect();
    assert!(
        accepted.is_empty(),
        "these are not language tags but were accepted: {accepted:?}"
    );
}

#[test]
fn a_list_of_tags_is_not_one_tag() {
    for raw in ["en+fr", "pt,it", "ru/ko"] {
        assert!(Language::parse(raw).is_err(), "'{raw}' is not one tag");
    }
}

#[test]
fn a_subtag_longer_than_eight_characters_is_rejected() {
    for raw in ["en-abcdefghi", "pt-BR-abcdefghijkl", "de-123456789"] {
        assert!(
            Language::parse(raw).is_err(),
            "'{raw}' has a subtag longer than BCP-47 allows"
        );
    }
}

#[test]
fn a_script_placed_after_the_region_is_rejected() {
    // Script comes before region; a four-letter subtag after the region is invalid.
    for raw in ["sr-RS-Latn", "zh-HK-Hant", "uz-UZ-Cyrl"] {
        assert!(
            Language::parse(raw).is_err(),
            "'{raw}' has its script in the wrong place and must not be accepted"
        );
    }
}

#[test]
fn a_rejected_tag_is_a_validation_error() {
    let err = Language::parse("en--US").unwrap_err();
    assert_eq!(err.kind, ErrorKind::Validation);
    assert!(!err.retryable);
    assert!("1x".parse::<Language>().is_err());
}
