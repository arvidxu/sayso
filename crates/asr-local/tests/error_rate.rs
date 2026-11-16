use asr_local::error_rate::{ErrorCount, character_errors, comparable_words, word_errors};

#[test]
fn text_that_differs_only_in_case_punctuation_and_spacing_has_no_errors() {
    let pairs = [
        (
            "Wir treffen uns am Donnerstag.",
            "wir treffen uns  am donnerstag",
        ),
        ("¿Dónde está, por favor?", "dónde está por favor"),
        ("It's 5 o'clock", "it s 5 o clock"),
    ];
    for (reference, recognised) in pairs {
        assert_eq!(word_errors(reference, recognised).errors, 0, "{reference}");
        assert_eq!(
            character_errors(reference, recognised).errors,
            0,
            "{reference}"
        );
    }
}

#[test]
fn in_writing_without_spaces_only_the_character_measure_ignores_punctuation() {
    // A comma splits the text into two "words", so the word measure is wrong for such languages.
    let (reference, recognised) = (
        "木曜日の午後に、会議をします。",
        "木曜日の午後に会議をします",
    );
    assert_eq!(character_errors(reference, recognised).errors, 0);
    assert!(word_errors(reference, recognised).errors > 0);
}

#[test]
fn accents_and_different_letters_count_as_errors() {
    assert_eq!(word_errors("café", "cafe").errors, 1);
    assert_eq!(character_errors("café", "cafe").errors, 1);
}

#[test]
fn replaced_missing_and_extra_words_each_count_once() {
    let reference = "sininen talo on järven rannalla";
    assert_eq!(
        word_errors(reference, reference),
        ErrorCount {
            reference: 5,
            errors: 0
        }
    );
    assert_eq!(
        word_errors(reference, "sininen auto on järven rannalla").errors,
        1
    );
    assert_eq!(
        word_errors(reference, "sininen talo järven rannalla").errors,
        1
    );
    assert_eq!(
        word_errors(reference, "sininen talo on on järven rannalla").errors,
        1
    );
    assert_eq!(word_errors(reference, "punainen auto järven").errors, 4);
    assert_eq!(word_errors(reference, "").errors, 5);
}

#[test]
fn characters_are_compared_without_regard_to_word_breaks() {
    // One wrong character out of six.
    let count = character_errors("今天天气很好", "今天天汽很好");
    assert_eq!(
        count,
        ErrorCount {
            reference: 6,
            errors: 1
        }
    );
    // The same letters split differently are no character errors, but are word errors.
    assert_eq!(character_errors("ice cream", "icecream").errors, 0);
    assert_eq!(word_errors("ice cream", "icecream").errors, 2);
}

#[test]
fn the_rate_is_errors_per_reference_unit_and_can_pass_one() {
    assert_eq!(word_errors("a b c d", "a b x d").rate(), Some(0.25));
    assert_eq!(word_errors("a", "x y z").rate(), Some(3.0));
    assert_eq!(word_errors("", "anything").rate(), None);
    assert_eq!(word_errors("...", "").rate(), None);
}

#[test]
fn counts_add_up_across_recordings() {
    let total = word_errors("a b", "a x").plus(&word_errors("c d e", "c d e"));
    assert_eq!(
        total,
        ErrorCount {
            reference: 5,
            errors: 1
        }
    );
    assert_eq!(total.rate(), Some(0.2));
}

#[test]
fn words_are_split_at_anything_that_is_not_a_letter_or_digit() {
    assert_eq!(
        comparable_words("Año 2026: ¡listo!"),
        ["año", "2026", "listo"]
    );
    assert!(comparable_words(" —,。 ").is_empty());
    assert_eq!(comparable_words("नमस्ते दुनिया"), ["नमस्ते", "दुनिया"]);
}
