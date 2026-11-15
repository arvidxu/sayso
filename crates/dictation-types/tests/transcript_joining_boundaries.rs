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
fn an_opening_quote_followed_by_a_space_opens_a_quote() {
    for space in [" ", "\u{00A0}", "\u{202F}"] {
        let quoted = format!("«{space}Je viens demain.{space}»");
        assert_eq!(
            join(&["Il a dit :", &quoted]),
            format!("Il a dit : {quoted}"),
            "{space:?}"
        );
        let opened = format!("«{space}Je viens");
        assert_eq!(
            join(&["Il a dit :", &opened]),
            format!("Il a dit : {opened}"),
            "{space:?}"
        );
    }
}

#[test]
fn an_apostrophe_starting_a_word_starts_that_word_and_opens_no_quote() {
    assert_eq!(
        join(&["Hy het 'n hond.", "'n Mens moet dit sien."]),
        "Hy het 'n hond. 'n Mens moet dit sien."
    );
    assert_eq!(
        join(&["Ik werk 's morgens", "'s avonds niet"]),
        "Ik werk 's morgens 's avonds niet"
    );
    assert_eq!(
        join(&["Back in the '90s", "'cause we could"]),
        "Back in the '90s 'cause we could"
    );
    // A quote the segment closes is still a quote.
    assert_eq!(join(&["He said", "'hello' twice"]), "He said 'hello' twice");
}

#[test]
fn nothing_goes_after_or_before_a_syllable_separator() {
    assert_eq!(join(&["ཁྱེད་རང་", "ག་འདྲ་འདུག"]), "ཁྱེད་རང་ག་འདྲ་འདུག");
    assert_eq!(join(&["ཁྱེད", "་རང་"]), "ཁྱེད་རང་");
    // A phrase ending in a shad is still followed by a space.
    assert_eq!(join(&["བཀྲ་ཤིས་བདེ་ལེགས།", "ཁྱེད་རང་"]), "བཀྲ་ཤིས་བདེ་ལེགས། ཁྱེད་རང་");
}

#[test]
fn cjk_brackets_take_the_spacing_of_the_letters_next_to_them() {
    assert_eq!(
        join(&["그는", "《해리 포터》를 읽었다"]),
        "그는 《해리 포터》를 읽었다"
    );
    assert_eq!(join(&["그는 《", "해리 포터》"]), "그는 《해리 포터》");
    assert_eq!(join(&["他读了", "《哈利·波特》"]), "他读了《哈利·波特》");
    assert_eq!(
        join(&["「こんにちは」", "と言った"]),
        "「こんにちは」と言った"
    );
    assert_eq!(join(&["他说", "「"]), "他说「");
}
