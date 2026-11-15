use dictation_types::{TranscriptSegment, join_segment_texts};

fn join(parts: &[&str]) -> String {
    let segments: Vec<TranscriptSegment> = parts
        .iter()
        .enumerate()
        .map(|(i, text)| TranscriptSegment::new(*text, i as u64, i as u64 + 1).unwrap())
        .collect();
    join_segment_texts(&segments)
}

#[test]
fn no_space_goes_before_a_closing_quote_or_bracket() {
    assert_eq!(join(&["He said \"hi", "\""]), "He said \"hi\"");
    assert_eq!(join(&["„Hallo", "“"]), "„Hallo“");
    assert_eq!(join(&["«Bonjour", "»"]), "«Bonjour»");
    assert_eq!(join(&["(see below", ")"]), "(see below)");
    assert_eq!(join(&["[note", "]"]), "[note]");
}

#[test]
fn a_space_goes_before_an_opening_quote_or_bracket_after_a_word() {
    assert_eq!(join(&["Er sagte", "„Hallo“"]), "Er sagte „Hallo“");
    assert_eq!(join(&["She said", "\"hi\""]), "She said \"hi\"");
    assert_eq!(join(&["Il a dit", "«oui»"]), "Il a dit «oui»");
    assert_eq!(join(&["see", "(below)"]), "see (below)");
}

#[test]
fn a_space_goes_after_a_closing_quote_or_bracket_before_a_word() {
    assert_eq!(join(&["(world)", "hello"]), "(world) hello");
    assert_eq!(join(&["\"hi\"", "she said"]), "\"hi\" she said");
    assert_eq!(join(&["„Hallo“", "sagte er"]), "„Hallo“ sagte er");
    assert_eq!(join(&["‘yes’", "and"]), "‘yes’ and");
    assert_eq!(join(&["‘yes!’", "and"]), "‘yes!’ and");
    assert_eq!(join(&["the dogs’", "bowls"]), "the dogs’ bowls");
    assert_eq!(join(&["don’t", "go"]), "don’t go");
    assert_eq!(join(&["‘yes", "’"]), "‘yes’");
}

#[test]
fn numbers_on_both_sides_of_a_pause_stay_two_words() {
    // A pause inside a number is rare: `12.` then `3` are two words.
    assert_eq!(
        join(&["The total is 12.", "3 of them were late."]),
        "The total is 12. 3 of them were late."
    );
    assert_eq!(
        join(&["In 2020,", "2021 and 2022"]),
        "In 2020, 2021 and 2022"
    );
    assert_eq!(join(&["٣٫", "١٤"]), "٣٫ ١٤");
    assert_eq!(join(&["Chapter 3.", "Then"]), "Chapter 3. Then");
}

#[test]
fn a_segment_that_starts_with_a_combining_mark_continues_the_word() {
    assert_eq!(join(&["cafe", "\u{301}s"]), "cafe\u{301}s");
}

#[test]
fn words_in_scripts_without_data_are_separated_too() {
    // Santali in Ol Chiki, Odia and Dhivehi in Thaana all put spaces between words.
    assert_eq!(join(&["ᱥᱟᱱᱛᱟᱲᱤ", "ᱯᱟᱹᱨᱥᱤ"]), "ᱥᱟᱱᱛᱟᱲᱤ ᱯᱟᱹᱨᱥᱤ");
    assert_eq!(join(&["ଓଡ଼ିଆ", "ଭାଷା"]), "ଓଡ଼ିଆ ଭାଷା");
    assert_eq!(join(&["ދިވެހި", "ބަސް"]), "ދިވެހި ބަސް");
}

#[test]
fn symbols_currency_signs_and_dashes_are_spaced_like_words() {
    let cases: [(&[&str], &str); 10] = [
        (
            &["I'm 100%", "sure about that."],
            "I'm 100% sure about that.",
        ),
        (&["I paid", "$5 for it"], "I paid $5 for it"),
        (&["Das kostet 5 €", "pro Stück"], "Das kostet 5 € pro Stück"),
        (&["It is 30°", "outside"], "It is 30° outside"),
        (
            &["The temperature is", "-5 degrees"],
            "The temperature is -5 degrees",
        ),
        (&["Wait —", "no"], "Wait — no"),
        (&["Wait", "— no"], "Wait — no"),
        (&["see", "#3"], "see #3"),
        (&["great", "😀"], "great 😀"),
        (&["😀", "great"], "😀 great"),
    ];
    for (parts, expected) in cases {
        assert_eq!(join(parts), expected, "{parts:?}");
    }
}

#[test]
fn straight_single_quotes_open_and_close_like_any_quote() {
    assert_eq!(join(&["He said", "'hello'"]), "He said 'hello'");
    assert_eq!(join(&["He said 'hello", "'"]), "He said 'hello'");
    assert_eq!(join(&["'hello'", "he said"]), "'hello' he said");
    assert_eq!(join(&["the dogs'", "bowls"]), "the dogs' bowls");
    assert_eq!(join(&["don't", "go"]), "don't go");
    assert_eq!(join(&["'don't", "'"]), "'don't'");
}

#[test]
fn quotes_that_close_with_the_character_they_open_with_are_paired() {
    // Swedish and Finnish open and close with the same character.
    assert_eq!(join(&["Han sa", "”Hej”"]), "Han sa ”Hej”");
    assert_eq!(join(&["Han sa ”Hej", "”"]), "Han sa ”Hej”");
    assert_eq!(join(&["Han sa ”Hej”", "och gick"]), "Han sa ”Hej” och gick");
    assert_eq!(join(&["Han sa »Hej»", "och gick"]), "Han sa »Hej» och gick");
    assert_eq!(join(&["Han sa", "»Hej»"]), "Han sa »Hej»");
    assert_eq!(join(&["»Hej", "»"]), "»Hej»");
    assert_eq!(
        join(&["Hän sanoi ’hei’", "ja lähti"]),
        "Hän sanoi ’hei’ ja lähti"
    );
    // A closing quote nobody opened still ends the text before it.
    assert_eq!(join(&["she said hi”", "and left"]), "she said hi” and left");
    assert_eq!(join(&["the dogs", "’"]), "the dogs’");
    assert_eq!(
        join(&["He said:“hi", "”, then left"]),
        "He said:“hi”, then left"
    );
    assert_eq!(join(&["and so on", "”, she said"]), "and so on”, she said");
}

#[test]
fn nothing_goes_after_an_opening_quote_or_bracket() {
    assert_eq!(join(&["(", "see below)"]), "(see below)");
    assert_eq!(join(&["He said", "“", "hello", "”"]), "He said “hello”");
    assert_eq!(join(&["He said “", "hi”"]), "He said “hi”");
    assert_eq!(join(&["Er sagte „", "Hallo“"]), "Er sagte „Hallo“");
    assert_eq!(join(&["¿", "Qué?"]), "¿Qué?");
}

#[test]
fn modifier_letters_start_words_and_are_spaced() {
    // The ʻokina of Hawaiian and the glottal stop letter of many languages are letters.
    assert_eq!(join(&["aloha", "ʻohana"]), "aloha ʻohana");
    assert_eq!(join(&["ka", "ʼa"]), "ka ʼa");
}

#[test]
fn guillemets_pointing_inwards_open_and_close_like_any_quote() {
    assert_eq!(join(&["Er sagte", "»Hallo«"]), "Er sagte »Hallo«");
    assert_eq!(join(&["»Hallo", "«"]), "»Hallo«");
    assert_eq!(join(&["»Hallo«", "sagte er"]), "»Hallo« sagte er");
    assert_eq!(join(&["Han sagde", "»hej«"]), "Han sagde »hej«");
    assert_eq!(join(&["«oui»", "a-t-il dit"]), "«oui» a-t-il dit");
}

#[test]
fn sentence_endings_of_other_scripts_are_followed_by_a_space() {
    let cases: [(&[&str], &str); 8] = [
        (&["مرحبا،", "كيف"], "مرحبا، كيف"),
        (&["كيف حالك؟", "أنا"], "كيف حالك؟ أنا"),
        (&["آپ کیسے ہیں۔", "میں"], "آپ کیسے ہیں۔ میں"),
        (&["मैं ठीक हूँ।", "आप"], "मैं ठीक हूँ। आप"),
        (&["আমি ভালো আছি।", "তুমি"], "আমি ভালো আছি। তুমি"),
        (&["Բարեւ։", "Ինչպես"], "Բարեւ։ Ինչպես"),
        (&["ሰላም።", "እንዴት"], "ሰላም። እንዴት"),
        (&["Wait…", "what"], "Wait… what"),
    ];
    for (parts, expected) in cases {
        assert_eq!(join(parts), expected);
    }
}
