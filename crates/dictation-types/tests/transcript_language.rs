mod support;

use dictation_types::TranscriptSegment;
use support::{from_segments, lang, seg, transcript};

fn seg_in(text: &str, tag: &str) -> TranscriptSegment {
    seg(text).with_language(lang(tag))
}

#[test]
fn segments_all_reported_as_french_give_french_to_an_english_french_user() {
    let segments = vec![seg_in("bonjour", "fr"), seg_in("tout le monde", "fr")];
    for spec in ["en+fr", "fr+en"] {
        assert_eq!(
            from_segments(segments.clone(), spec).dominant_language(),
            Some(lang("fr"))
        );
    }
}

#[test]
fn reported_languages_decide_between_languages_that_share_a_script() {
    let cases = [
        ("pt+it", "it", vec!["buongiorno", "come stai oggi"]),
        ("it+pt", "pt", vec!["bom dia", "como você está"]),
        ("id+ms", "ms", vec!["selamat pagi", "apa khabar"]),
        ("ru+uk+bg", "uk", vec!["доброго ранку", "як справи"]),
        ("hi+mr", "mr", vec!["तुम्ही कसे आहात"]),
        ("ar+fa+ur", "ur", vec!["آپ کیسے ہیں"]),
        ("zh+yue", "yue", vec!["今日天氣好好"]),
    ];
    for (spec, reported, texts) in cases {
        let segments = texts.iter().map(|t| seg_in(t, reported)).collect();
        assert_eq!(
            from_segments(segments, spec).dominant_language(),
            Some(lang(reported)),
            "{spec}"
        );
    }
}

#[test]
fn reported_languages_decide_whatever_letters_the_text_is_written_in() {
    // Bengali typed out in Latin letters.
    let segments = vec![seg_in("ami aaj bajare jabo", "bn")];
    for spec in ["en+bn", "bn+en"] {
        assert_eq!(
            from_segments(segments.clone(), spec).dominant_language(),
            Some(lang("bn"))
        );
    }

    // Serbian in Latin letters for a user whose tag does not say which script they use.
    let serbian = vec![seg_in("dobar dan kako ste", "sr")];
    assert_eq!(
        from_segments(serbian, "en+sr").dominant_language(),
        Some(lang("sr"))
    );
}

#[test]
fn reported_languages_are_weighted_by_the_amount_of_text() {
    let mostly_french = vec![
        seg_in("ok", "en"),
        seg_in("je pense que nous devrions partir maintenant", "fr"),
        seg_in("yes", "en"),
    ];
    for spec in ["en+fr", "fr+en"] {
        assert_eq!(
            from_segments(mostly_french.clone(), spec).dominant_language(),
            Some(lang("fr")),
            "{spec}"
        );
    }

    let mostly_korean = vec![
        seg_in("да", "ru"),
        seg_in("오늘 오후에 회의가 있어서 조금 늦을 것 같아요", "ko"),
        seg_in("oui", "fr"),
        seg_in("нет", "ru"),
    ];
    assert_eq!(
        from_segments(mostly_korean, "ru+ko+fr").dominant_language(),
        Some(lang("ko"))
    );
}

#[test]
fn many_short_segments_outweigh_one_shorter_segment() {
    let segments = vec![
        seg_in("come", "it"),
        seg_in("stai", "it"),
        seg_in("oggi", "it"),
        seg_in("amico", "it"),
        seg_in("obrigado", "pt"),
    ];
    assert_eq!(
        from_segments(segments, "pt+it").dominant_language(),
        Some(lang("it"))
    );
}

#[test]
fn a_reported_language_resolves_to_the_users_own_variant() {
    let segments = vec![seg_in("bom dia a todos", "pt")];
    assert_eq!(
        from_segments(segments.clone(), "it+pt-BR").dominant_language(),
        Some(lang("pt-BR"))
    );
    let regional = vec![seg_in("bom dia a todos", "pt-PT")];
    assert_eq!(
        from_segments(regional, "it+pt-BR").dominant_language(),
        Some(lang("pt-BR"))
    );
    let chinese = vec![seg_in("今日天氣非常好", "zh")];
    assert_eq!(
        from_segments(chinese, "en+zh-Hant-HK").dominant_language(),
        Some(lang("zh-Hant-HK"))
    );
}

#[test]
fn regional_variants_reported_separately_count_as_one_language() {
    let segments = vec![
        seg_in("bom dia", "pt-BR"),
        seg_in("buongiorno amici", "it"),
        seg_in("tudo bem", "pt-PT"),
        seg_in("obrigado", "pt"),
    ];
    assert_eq!(
        from_segments(segments, "it+pt").dominant_language(),
        Some(lang("pt"))
    );
}

#[test]
fn a_reported_language_the_user_did_not_list_is_still_reported() {
    let segments = vec![seg_in("guten Morgen zusammen", "de")];
    assert_eq!(
        from_segments(segments, "pt+it").dominant_language(),
        Some(lang("de"))
    );
}

#[test]
fn segments_without_a_report_do_not_outvote_those_with_one() {
    let segments = vec![
        seg("this part came without any language report at all"),
        seg_in("bonjour", "fr"),
    ];
    assert_eq!(
        from_segments(segments, "en+fr").dominant_language(),
        Some(lang("fr"))
    );
}

#[test]
fn without_reports_there_is_no_dominant_language() {
    let unreported = from_segments(vec![seg("안녕하세요"), seg("여러분")], "ru+ko+fr");
    assert_eq!(unreported.dominant_language(), None);
    assert_eq!(
        transcript("I will go to the market", "en+bn").dominant_language(),
        None
    );
}

#[test]
fn with_no_text_and_no_reports_there_is_no_dominant_language() {
    for spec in ["en+fr", "pt+it", "bn", "ru+ko+fr"] {
        assert_eq!(transcript("", spec).dominant_language(), None, "{spec}");
        assert_eq!(
            transcript("   \n", spec).dominant_language(),
            None,
            "{spec}"
        );
        assert_eq!(
            from_segments(Vec::new(), spec).dominant_language(),
            None,
            "{spec}"
        );
        assert_eq!(
            from_segments(vec![seg(""), seg(" ")], spec).dominant_language(),
            None,
            "{spec}"
        );
    }
}

#[test]
fn reports_attached_to_empty_segments_decide_nothing() {
    let segments = vec![seg_in("", "fr"), seg_in("   ", "fr")];
    assert_eq!(from_segments(segments, "en+fr").dominant_language(), None);
}

#[test]
fn two_reported_languages_make_a_transcript_code_mixed() {
    let segments = vec![seg_in("bom dia", "pt"), seg_in("come stai", "it")];
    assert!(from_segments(segments, "pt+it").is_code_mixed());
}

#[test]
fn two_reported_variants_of_one_language_are_not_code_mixed() {
    let segments = vec![seg_in("bom dia", "pt-BR"), seg_in("tudo bem", "pt-PT")];
    assert!(!from_segments(segments, "pt-BR+pt-PT").is_code_mixed());
}

#[test]
fn the_language_spoken_longest_wins_however_it_is_written() {
    let timed = |text: &str, tag: &str, start_ms: u64, end_ms: u64| {
        TranscriptSegment::new(text, start_ms, end_ms)
            .unwrap()
            .with_language(lang(tag))
    };
    // Eight characters spoken for three seconds outweigh more letters spoken for one.
    let segments = vec![
        timed("今天我们开会讨论", "zh", 0, 3_000),
        timed("the roadmap today", "en", 3_000, 4_000),
    ];
    assert_eq!(
        from_segments(segments, "en+zh").dominant_language(),
        Some(lang("zh"))
    );

    // Once any segment is timed, an untimed one does not count its letters against time.
    let mixed = vec![
        timed("ok", "en", 0, 5),
        timed("je pense que nous devrions partir", "fr", 0, 0),
    ];
    assert_eq!(
        from_segments(mixed, "en+fr").dominant_language(),
        Some(lang("en"))
    );
}
