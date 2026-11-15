use dictation_types::{AsrCaps, Language, Languages};

fn lang(tag: &str) -> Language {
    Language::parse(tag).expect("language tag")
}

#[test]
fn a_withdrawn_code_and_its_replacement_are_the_same_language() {
    for (old, current) in [
        ("iw", "he"),
        ("in", "id"),
        ("ji", "yi"),
        ("jw", "jv"),
        ("mo", "ro"),
    ] {
        assert!(lang(old).matches(&lang(current)), "{old} and {current}");
        assert!(lang(current).matches(&lang(old)), "{current} and {old}");
    }
    // `tl` is still a valid code; locale data writes `fil` for both.
    assert!(lang("tl").matches(&lang("fil-PH")));
    assert!(lang("fil").matches(&lang("tl")));
}

#[test]
fn a_macrolanguage_and_the_language_its_code_stands_for_are_the_same_language() {
    for (macrolanguage, individual) in [
        ("ar", "arb"),
        ("zh", "cmn"),
        ("ms", "zsm"),
        ("fa", "pes"),
        ("sw", "swh"),
        ("et", "ekk"),
        ("lv", "lvs"),
        ("mn", "khk"),
        ("uz", "uzn"),
        ("ne", "npi"),
        ("sq", "als"),
        ("ku", "kmr"),
        ("no", "nb"),
    ] {
        assert!(
            lang(macrolanguage).matches(&lang(individual)),
            "{macrolanguage} and {individual}"
        );
        assert!(
            lang(&format!("{individual}-x-test")).matches(&lang(macrolanguage)),
            "{individual} and {macrolanguage}"
        );
        assert_eq!(lang(individual).canonical_primary(), macrolanguage);
    }
    // The extended language form names the individual language too.
    assert!(lang("ar-arb").matches(&lang("ar-EG")));
    assert!(lang("ms-zsm-MY").matches(&lang("ms")));
    // An engine declared with either code serves users of the other.
    let arabic = AsrCaps::new(vec![lang("ar")], 16_000, true);
    assert!(arabic.supports_language(&lang("arb")));
    assert!(!arabic.supports_language(&lang("arz")));
    let malay = AsrCaps::new(vec![lang("zsm")], 16_000, true);
    assert!(malay.supports_language(&lang("ms-MY")));
}

#[test]
fn other_members_of_a_macrolanguage_are_languages_of_their_own() {
    for (macrolanguage, member) in [
        ("zh", "yue"),
        ("zh", "nan"),
        ("ar", "arz"),
        ("ar", "ary"),
        ("sw", "swc"),
        ("fa", "prs"),
        ("ku", "ckb"),
        ("no", "nn"),
    ] {
        assert!(
            !lang(macrolanguage).matches(&lang(member)),
            "{macrolanguage} and {member}"
        );
    }
}

#[test]
fn the_canonical_tag_uses_the_current_code_and_keeps_the_rest() {
    for (given, canonical) in [
        ("iw-IL", "he-IL"),
        ("arb-EG", "ar-EG"),
        ("zh-cmn-Hant-TW", "zh-Hant-TW"),
        ("cmn-Hans", "zh-Hans"),
        ("zsm", "ms"),
        ("nb-NO", "no-NO"),
        ("pt-BR", "pt-BR"),
        ("zh-yue-HK", "yue-HK"),
        ("sl-rozaj-biske", "sl-rozaj-biske"),
    ] {
        assert_eq!(lang(given).canonical().code(), canonical, "{given}");
    }
}

#[test]
fn a_language_and_its_predominant_form_are_the_same_language() {
    assert!(lang("no").matches(&lang("nb-NO")));
    assert!(lang("zh-CN").matches(&lang("cmn")));
    assert!(lang("zh-cmn-Hant-TW").matches(&lang("cmn-Hans")));
    assert!(
        !lang("no").matches(&lang("nn")),
        "Nynorsk has its own engines and spelling"
    );
    assert!(!lang("zh").matches(&lang("yue")));
}

#[test]
fn a_tag_keeps_the_spelling_it_was_given() {
    assert_eq!(lang("iw-IL").code(), "iw-IL");
    assert_eq!(lang("iw-IL").primary(), "iw");
    assert_eq!(lang("iw-IL").canonical_primary(), "he");
}

#[test]
fn an_engine_declared_with_one_code_serves_users_who_chose_the_other() {
    let norwegian = AsrCaps::new(vec![lang("no")], 16_000, true);
    assert!(norwegian.supports_language(&lang("nb-NO")));
    assert!(
        norwegian
            .check_languages(&Languages::single(lang("nb")))
            .is_empty()
    );

    let hebrew = AsrCaps::new(vec![lang("he")], 16_000, true);
    assert!(hebrew.supports_language(&lang("iw")));
}

#[test]
fn two_codes_for_one_language_do_not_make_a_user_multilingual() {
    let list = Languages::parse("he+iw+no+nb").expect("list");
    assert_eq!(list.len(), 4);
    assert_eq!(list.distinct_languages(), 2);
}
