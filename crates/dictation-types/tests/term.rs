mod support;

use std::collections::HashSet;

use dictation_types::{
    ErrorKind, FillerWord, ReplacementKind, ReplacementRule, RuleId, Term, TermId, TermKind,
};
use support::lang;

fn term(canonical: &str, tag: &str) -> Term {
    Term::new(
        TermId::new("term-1"),
        canonical,
        TermKind::ProperNoun,
        lang(tag),
    )
    .unwrap()
}

#[test]
fn a_term_matches_itself_and_returns_the_canonical_spelling() {
    for (canonical, tag) in [
        ("Zentrava", "en"),
        ("São Paulo", "pt-BR"),
        ("Москва", "ru"),
        ("Θεσσαλονίκη", "el"),
        ("서울", "ko"),
        ("東京", "ja"),
        ("ঢাকা", "bn"),
        ("القاهرة", "ar"),
        ("กรุงเทพ", "th"),
    ] {
        assert_eq!(
            term(canonical, tag).restore_spelling(canonical),
            Some(canonical)
        );
    }
}

#[test]
fn matching_ignores_letter_case_in_accented_latin() {
    let cases = [
        (
            "São Paulo",
            "pt-BR",
            vec!["são paulo", "SÃO PAULO", "sÃO pAULO"],
        ),
        ("Łódź", "pl", vec!["łódź", "ŁÓDŹ"]),
        ("Ångström", "sv", vec!["ångström", "ÅNGSTRÖM"]),
        ("Élysée", "fr", vec!["élysée", "ÉLYSÉE"]),
        ("Đà Nẵng", "vi", vec!["đà nẵng", "ĐÀ NẴNG"]),
        ("Zentrava", "en", vec!["zentrava", "ZENTRAVA", "zEnTrAvA"]),
    ];
    for (canonical, tag, candidates) in cases {
        let term = term(canonical, tag);
        for candidate in candidates {
            assert_eq!(
                term.restore_spelling(candidate),
                Some(canonical),
                "{candidate}"
            );
        }
    }
}

#[test]
fn matching_ignores_letter_case_in_cyrillic_greek_and_armenian() {
    let cases = [
        ("Москва", "ru", vec!["москва", "МОСКВА", "мОСКВА"]),
        ("Київ", "uk", vec!["київ", "КИЇВ"]),
        ("Θεσσαλονίκη", "el", vec!["θεσσαλονίκη", "ΘΕΣΣΑΛΟΝΊΚΗ"]),
        ("Οδυσσέας", "el", vec!["οδυσσέας", "ΟΔΥΣΣΈΑΣ"]),
        ("Երևան", "hy", vec!["երևան"]),
    ];
    for (canonical, tag, candidates) in cases {
        let term = term(canonical, tag);
        for candidate in candidates {
            assert_eq!(
                term.restore_spelling(candidate),
                Some(canonical),
                "{candidate}"
            );
        }
    }
}

#[test]
fn matching_ignores_letter_case_for_the_german_sharp_s() {
    // The capital form of "ß" is written "SS".
    let street = term("Straße", "de");
    assert_eq!(street.restore_spelling("straße"), Some("Straße"));
    assert_eq!(street.restore_spelling("STRASSE"), Some("Straße"));
}

#[test]
fn matching_ignores_letter_case_for_the_turkish_dotted_i() {
    // The capital of "i" in Turkish is "İ".
    let city = term("İstanbul", "tr");
    assert_eq!(city.restore_spelling("İSTANBUL"), Some("İstanbul"));
    assert_eq!(city.restore_spelling("istanbul"), Some("İstanbul"));
}

#[test]
fn an_alias_is_written_back_as_the_canonical_spelling() {
    let cases = [
        (
            "Zentrava",
            "en",
            vec!["kubernetis", "cooper netties"],
            "COOPER NETTIES",
        ),
        ("São Paulo", "pt-BR", vec!["sao paulo", "sampa"], "Sampa"),
        ("Москва", "ru", vec!["масква"], "МАСКВА"),
        ("東京", "ja", vec!["とうきょう", "トウキョウ"], "とうきょう"),
        ("서울", "ko", vec!["seoul"], "SEOUL"),
        ("ঢাকা", "bn", vec!["dhaka", "ঢাকা শহর"], "Dhaka"),
    ];
    for (canonical, tag, aliases, heard) in cases {
        let term = term(canonical, tag).with_aliases(aliases.clone());
        assert_eq!(term.restore_spelling(heard), Some(canonical), "{heard}");
        for alias in aliases {
            assert_eq!(term.restore_spelling(alias), Some(canonical), "{alias}");
        }
    }
}

#[test]
fn something_that_is_neither_the_term_nor_an_alias_does_not_match() {
    let term = term("São Paulo", "pt-BR").with_aliases(["sampa"]);
    for candidate in [
        "São",
        "Paulo",
        "São Paulo city",
        "Rio de Janeiro",
        "",
        "Москва",
    ] {
        assert_eq!(term.restore_spelling(candidate), None, "{candidate:?}");
    }
}

#[test]
fn a_disabled_term_matches_nothing() {
    let mut disabled = term("Москва", "ru").with_aliases(["масква"]);
    disabled.enabled = false;
    for candidate in ["Москва", "москва", "МОСКВА", "масква"] {
        assert_eq!(disabled.restore_spelling(candidate), None, "{candidate}");
    }
    disabled.enabled = true;
    assert_eq!(disabled.restore_spelling("москва"), Some("Москва"));
}

#[test]
fn a_blank_term_is_rejected() {
    for blank in ["", " ", "\t\n", "\u{3000}"] {
        let result = Term::new(TermId::new("t"), blank, TermKind::Word, lang("ko"));
        assert_eq!(result.unwrap_err().kind, ErrorKind::Validation, "{blank:?}");
    }
}

#[test]
fn a_term_can_be_in_any_language() {
    for tag in [
        "pt-BR",
        "it",
        "bn-IN",
        "ru",
        "ko",
        "fr",
        "sw",
        "yue",
        "zh-Hant-HK",
        "haw",
    ] {
        let made = Term::new(TermId::new("t"), "x", TermKind::Word, lang(tag)).unwrap();
        assert_eq!(made.language, lang(tag));
        assert!(made.enabled);
        assert!(made.aliases.is_empty());
        assert_eq!(made.phonemes, None);
    }
}

#[test]
fn a_term_weight_is_never_negative() {
    assert_eq!(term("x", "it").with_weight(-3.0).weight, Some(0.0));
    assert_eq!(term("x", "it").with_weight(2.5).weight, Some(2.5));
}

#[test]
fn a_term_keeps_the_pronunciation_it_was_given() {
    assert_eq!(
        term("Nguyễn", "vi")
            .with_phonemes("ŋwiən")
            .phonemes
            .as_deref(),
        Some("ŋwiən")
    );
}

#[test]
fn an_empty_replacement_pattern_is_rejected() {
    for kind in [
        ReplacementKind::Exact,
        ReplacementKind::CaseInsensitive,
        ReplacementKind::Regex,
    ] {
        let result = ReplacementRule::new(RuleId::new("r"), "", "anything", kind);
        assert_eq!(result.unwrap_err().kind, ErrorKind::Validation, "{kind:?}");
    }
}

#[test]
fn a_replacement_may_replace_with_nothing() {
    let rule =
        ReplacementRule::new(RuleId::new("r"), "tipo assim", "", ReplacementKind::Exact).unwrap();
    assert_eq!(rule.pattern, "tipo assim");
    assert_eq!(rule.replacement, "");
    assert!(rule.enabled);
}

#[test]
fn a_replacement_rule_keeps_its_pattern_exactly() {
    let rule = ReplacementRule::new(
        RuleId::new("r"),
        " вопрос ",
        "?",
        ReplacementKind::CaseInsensitive,
    )
    .unwrap();
    assert_eq!(rule.pattern, " вопрос ");
    assert_eq!(rule.kind, ReplacementKind::CaseInsensitive);
}

#[test]
fn the_same_filler_spelling_in_two_languages_is_two_entries() {
    // "um" is a hesitation in English and an ordinary word in Portuguese; "e" is a filler in
    // Japanese romanisation and a conjunction in Italian.
    let english = FillerWord::new("um", lang("en"), true);
    let portuguese = FillerWord::new("um", lang("pt"), false);
    assert_ne!(english, portuguese);

    let set: HashSet<FillerWord> = [
        FillerWord::new("um", lang("en"), true),
        FillerWord::new("um", lang("pt"), true),
        FillerWord::new("eh", lang("it"), true),
        FillerWord::new("eh", lang("fr"), true),
        FillerWord::new("eh", lang("fr"), true),
    ]
    .into_iter()
    .collect();
    assert_eq!(set.len(), 4);
}

#[test]
fn a_filler_word_can_be_in_any_script() {
    for (word, tag) in [
        ("ну", "ru"),
        ("음", "ko"),
        ("えーと", "ja"),
        ("嗯", "zh"),
        ("يعني", "ar"),
        ("মানে", "bn"),
    ] {
        let filler = FillerWord::new(word, lang(tag), false);
        assert_eq!(filler.word, word);
        assert_eq!(filler.language, lang(tag));
    }
}
