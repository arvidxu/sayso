mod support;

use std::str::FromStr;

use dictation_types::{ErrorKind, Language, Languages};
use support::{lang, langs};

fn codes(list: &Languages) -> Vec<String> {
    list.iter().map(Language::code).collect()
}

#[test]
fn the_order_the_user_gave_is_preserved() {
    assert_eq!(codes(&langs("ru+ko+fr")), ["ru", "ko", "fr"]);
    assert_eq!(codes(&langs("fr+ko+ru")), ["fr", "ko", "ru"]);
    assert_eq!(codes(&langs("bn+en")), ["bn", "en"]);
    assert_eq!(codes(&langs("en+bn")), ["en", "bn"]);

    let built = Languages::new([lang("it"), lang("pt"), lang("ta"), lang("sw")]).unwrap();
    assert_eq!(codes(&built), ["it", "pt", "ta", "sw"]);
}

#[test]
fn the_first_listed_language_is_the_primary_one() {
    assert_eq!(langs("pt+it").primary(), &lang("pt"));
    assert_eq!(langs("it+pt").primary(), &lang("it"));
    assert_eq!(langs("ko+ru+fr").primary(), &lang("ko"));
}

#[test]
fn exact_duplicates_are_dropped_keeping_the_first_occurrence() {
    let list = langs("pt-BR+it+PT_br+it+pt-br");
    assert_eq!(codes(&list), ["pt-BR", "it"]);
    assert_eq!(list.len(), 2);

    let built = Languages::new([lang("ru"), lang("RU"), lang("ko"), lang("ru")]).unwrap();
    assert_eq!(codes(&built), ["ru", "ko"]);
}

#[test]
fn regional_variants_are_both_kept_in_the_list() {
    let list = langs("pt-BR+pt-PT");
    assert_eq!(codes(&list), ["pt-BR", "pt-PT"]);
    assert_eq!(list.len(), 2);
}

#[test]
fn two_regional_variants_of_one_language_are_not_multilingual() {
    for spec in [
        "pt-BR+pt-PT",
        "fr-FR+fr-CA+fr-BE",
        "zh-CN+zh-TW",
        "sr-Latn+sr-Cyrl",
    ] {
        let list = langs(spec);
        assert_eq!(list.distinct_languages(), 1, "{spec}");
        assert!(!list.is_multilingual(), "{spec} is one language");
    }
}

#[test]
fn a_single_language_is_not_multilingual() {
    for spec in ["pt", "bn-IN", "ko", "sw"] {
        assert!(!langs(spec).is_multilingual());
        assert_eq!(langs(spec).distinct_languages(), 1);
    }
}

#[test]
fn any_two_or_more_distinct_languages_are_multilingual() {
    for (spec, distinct) in [
        ("pt+it", 2),
        ("bn+en", 2),
        ("ru+ko+fr", 3),
        ("ta+sw+el+he", 4),
        ("pt-BR+pt-PT+it", 2),
        ("vi+th+id+ar+uk", 5),
    ] {
        let list = langs(spec);
        assert!(list.is_multilingual(), "{spec}");
        assert_eq!(list.distinct_languages(), distinct, "{spec}");
    }
}

#[test]
fn ten_languages_at_once_are_accepted() {
    let spec = "pt+it+bn+en+ru+ko+fr+ta+ar+sw";
    let list = langs(spec);
    assert_eq!(list.len(), 10);
    assert_eq!(list.distinct_languages(), 10);
    assert_eq!(list.code(), spec);
    assert_eq!(list.primary(), &lang("pt"));
    assert_eq!(list.as_slice().last(), Some(&lang("sw")));
}

#[test]
fn every_documented_separator_is_understood() {
    let expected = ["ru", "ko", "fr"];
    for spec in ["ru+ko+fr", "ru/ko/fr", "ru,ko,fr", "ru+ko/fr", "ru,ko+fr"] {
        assert_eq!(codes(&langs(spec)), expected, "{spec}");
    }
}

#[test]
fn spaces_around_tags_are_ignored() {
    assert_eq!(codes(&langs("ru, ko, fr")), ["ru", "ko", "fr"]);
    assert_eq!(codes(&langs(" pt-BR + it ")), ["pt-BR", "it"]);
    assert_eq!(codes(&langs("bn / en")), ["bn", "en"]);
}

#[test]
fn empty_items_are_rejected() {
    for spec in [
        "", " ", "+", ",", "/", "en+", "+en", "pt++it", "ru,,ko", "bn/", "/bn", "ko+ +fr",
    ] {
        assert!(
            Languages::parse(spec).is_err(),
            "'{spec}' contains an empty item and must be rejected"
        );
    }
}

#[test]
fn one_malformed_tag_rejects_the_whole_list() {
    for spec in ["pt+1x", "ru+ko+fr-", "bn+e", "it+pt--BR"] {
        let err = Languages::parse(spec).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Validation, "{spec}");
    }
}

#[test]
fn a_list_cannot_be_built_from_nothing() {
    let err = Languages::new(Vec::<Language>::new()).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Validation);
}

#[test]
fn a_list_is_never_empty() {
    for spec in ["pt", "pt+it", "ru+ko+fr"] {
        let list = langs(spec);
        assert!(!list.is_empty());
        assert_ne!(list.len(), 0);
        assert_eq!(list.len(), list.as_slice().len());
    }
}

#[test]
fn a_single_language_becomes_a_list_of_one() {
    let from = Languages::from(lang("bn-IN"));
    let single = Languages::single(lang("bn-IN"));
    assert_eq!(from, single);
    assert_eq!(codes(&single), ["bn-IN"]);
    assert_eq!(single.primary(), &lang("bn-IN"));
}

#[test]
fn the_list_is_written_with_plus_signs_and_reads_back_the_same() {
    for spec in [
        "pt-BR+it+ja",
        "bn+en",
        "ru+ko+fr",
        "zh-Hant-HK+yue+en-GB",
        "sw",
    ] {
        let list = langs(spec);
        assert_eq!(list.code(), spec);
        assert_eq!(list.to_string(), spec);
        assert_eq!(Languages::from_str(&list.to_string()).unwrap(), list);
        assert_eq!(spec.parse::<Languages>().unwrap(), list);
    }
}

#[test]
fn containment_holds_across_regional_and_script_variants() {
    let list = langs("pt-BR+sr-Latn+ko");
    for tag in ["pt", "pt-PT", "PT_br", "sr", "sr-Cyrl-RS", "ko-KR", "ko"] {
        assert!(list.contains(&lang(tag)), "list must contain {tag}");
    }
    for tag in ["it", "es-BR", "hr-Latn", "ja", "en"] {
        assert!(!list.contains(&lang(tag)), "list must not contain {tag}");
    }
}

#[test]
fn find_returns_the_users_own_variant() {
    let list = langs("pt-BR+it+zh-Hant-HK");
    assert_eq!(list.find(&lang("pt")), Some(&lang("pt-BR")));
    assert_eq!(list.find(&lang("pt-PT")), Some(&lang("pt-BR")));
    assert_eq!(list.find(&lang("PT_br")), Some(&lang("pt-BR")));
    assert_eq!(list.find(&lang("zh")), Some(&lang("zh-Hant-HK")));
    assert_eq!(list.find(&lang("zh-Hans-CN")), Some(&lang("zh-Hant-HK")));
    assert_eq!(list.find(&lang("it-CH")), Some(&lang("it")));
    assert_eq!(list.find(&lang("es")), None);
}

#[test]
fn find_prefers_the_identical_tag_over_an_earlier_variant() {
    let list = langs("pt-BR+pt-PT+fr-CA+fr-FR");
    assert_eq!(list.find(&lang("pt-PT")), Some(&lang("pt-PT")));
    assert_eq!(list.find(&lang("fr-FR")), Some(&lang("fr-FR")));
    assert_eq!(list.find(&lang("pt")), Some(&lang("pt-BR")));
    assert_eq!(list.find(&lang("fr-BE")), Some(&lang("fr-CA")));
}

#[test]
fn the_list_can_be_iterated_in_order_every_way() {
    let list = langs("ta+el+he");
    let by_iter: Vec<String> = list.iter().map(Language::code).collect();
    let mut by_ref = Vec::new();
    for language in &list {
        by_ref.push(language.code());
    }
    let by_slice: Vec<String> = list.as_slice().iter().map(Language::code).collect();
    assert_eq!(by_iter, ["ta", "el", "he"]);
    assert_eq!(by_ref, by_iter);
    assert_eq!(by_slice, by_iter);
}

#[test]
fn lists_with_the_same_languages_in_another_order_are_different_lists() {
    assert_ne!(langs("bn+en"), langs("en+bn"));
    assert_eq!(langs("bn+en"), langs("BN / EN"));
}

#[test]
fn no_combination_of_languages_is_treated_differently() {
    for (first, second) in [
        ("pt", "it"),
        ("bn", "en"),
        ("ru", "ko"),
        ("zh", "en"),
        ("sw", "am"),
        ("ta", "si"),
        ("he", "ar"),
        ("vi", "km"),
    ] {
        let list = langs(&format!("{first}+{second}"));
        assert_eq!(list.len(), 2);
        assert!(list.is_multilingual());
        assert_eq!(list.primary().code(), first);
        assert!(list.contains(&lang(first)));
        assert!(list.contains(&lang(second)));
        assert_eq!(list.code(), format!("{first}+{second}"));
    }
}
