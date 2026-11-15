use dictation_types::{Language, Term, TermId, TermKind, fold_case};

fn term(text: &str, tag: &str) -> Term {
    Term::new(
        TermId::new("t"),
        text,
        TermKind::Word,
        Language::parse(tag).unwrap(),
    )
    .unwrap()
}

#[test]
fn dotted_and_dotless_i_fold_with_plain_i() {
    assert_eq!(fold_case("İstanbul"), fold_case("istanbul"));
    assert_eq!(fold_case("ISTANBUL"), fold_case("ıstanbul"));
    assert_eq!(
        term("İstanbul", "tr").restore_spelling("ISTANBUL"),
        Some("İstanbul")
    );
}

#[test]
fn a_combining_dot_is_not_thrown_away() {
    // Lithuanian uses a dot above on other letters; only the dotted capital I is special.
    assert_ne!(fold_case("e\u{307}"), fold_case("e"));
}

#[test]
fn surrounding_whitespace_does_not_prevent_a_match() {
    let t = term("Zentrava", "en");
    assert_eq!(t.restore_spelling(" zentrava"), Some("Zentrava"));
    assert_eq!(t.restore_spelling("ZENTRAVA\n"), Some("Zentrava"));
    assert_eq!(
        t.restore_spelling("zentrava."),
        None,
        "punctuation is the caller's to strip"
    );
}

#[test]
fn letter_case_is_ignored_in_scripts_that_have_it() {
    assert_eq!(
        term("Тбилиси", "ru").restore_spelling("ТБИЛИСИ"),
        Some("Тбилиси")
    );
    assert_eq!(term("Αθήνα", "el").restore_spelling("ΑΘΉΝΑ"), Some("Αθήνα"));
    assert_eq!(
        term("Straße", "de").restore_spelling("STRASSE"),
        Some("Straße")
    );
}

#[test]
fn the_capital_sharp_s_folds_with_the_small_one_and_with_double_s() {
    assert_eq!(fold_case("STRAẞE"), fold_case("Straße"));
    assert_eq!(fold_case("STRAẞE"), fold_case("STRASSE"));
    for written in ["STRAẞE", "straße", "STRASSE", "strasse"] {
        assert_eq!(
            term("Straße", "de").restore_spelling(written),
            Some("Straße"),
            "{written}"
        );
    }
    assert_eq!(term("GROẞ", "de").restore_spelling("groß"), Some("GROẞ"));
}
