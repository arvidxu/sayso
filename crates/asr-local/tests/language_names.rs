mod support;

use asr_local::LanguageNames;
use support::{ENGINE, options, speech_entry};

fn names() -> LanguageNames {
    LanguageNames::of(&speech_entry(
        "model-a",
        ENGINE,
        &["zh", "yue", "fil", "id", "de"],
        &[
            ("sampleRateHz", "16000"),
            ("languageName.zh", "Chinese"),
            ("languageName.yue", "Cantonese"),
            ("languageName.fil", "Filipino"),
            ("languageName.id", "Indonesian"),
            ("languageName.DE", "German"),
        ],
    ))
    .expect("every name is for a language")
}

#[test]
fn a_language_written_with_an_extended_subtag_is_hinted_as_itself() {
    assert_eq!(names().hint_for(&options("zh-yue")), Some("Cantonese"));
    assert_eq!(names().hint_for(&options("yue-HK")), Some("Cantonese"));
    assert_eq!(names().hint_for(&options("zh-Hant-TW")), Some("Chinese"));
    // Withdrawn and alternative codes find the entry written with the current one.
    assert_eq!(names().hint_for(&options("tl")), Some("Filipino"));
    assert_eq!(names().hint_for(&options("in")), Some("Indonesian"));
    assert_eq!(names().hint_for(&options("cmn")), Some("Chinese"));
    assert_eq!(names().hint_for(&options("de-AT")), Some("German"));
}

#[test]
fn no_hint_for_a_language_without_a_name_or_for_several_languages() {
    assert_eq!(names().hint_for(&options("sw")), None);
    assert_eq!(names().hint_for(&options("de+id")), None);
}
