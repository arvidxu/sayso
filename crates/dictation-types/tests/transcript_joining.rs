mod support;

use dictation_types::{TranscriptSegment, join_segment_texts};
use support::seg;

fn join(texts: &[&str]) -> String {
    let segments: Vec<TranscriptSegment> = texts.iter().map(|t| seg(t)).collect();
    join_segment_texts(&segments)
}

#[test]
fn english_words_are_joined_with_a_space() {
    assert_eq!(join(&["hello", "world"]), "hello world");
    assert_eq!(join(&["one", "two", "three"]), "one two three");
}

#[test]
fn words_of_every_spaced_script_are_joined_with_a_space() {
    let cases: [(&[&str], &str); 12] = [
        (&["bom", "dia"], "bom dia"),
        (&["buongiorno", "a", "tutti"], "buongiorno a tutti"),
        (&["привет", "мир"], "привет мир"),
        (&["안녕하세요", "여러분"], "안녕하세요 여러분"),
        (&["γεια", "σου"], "γεια σου"),
        (&["שלום", "עולם"], "שלום עולם"),
        (&["مرحبا", "بالعالم"], "مرحبا بالعالم"),
        (&["আমি", "আজ", "যাব"], "আমি আজ যাব"),
        (&["வணக்கம்", "நண்பர்களே"], "வணக்கம் நண்பர்களே"),
        (&["नमस्ते", "दुनिया"], "नमस्ते दुनिया"),
        (&["გამარჯობა", "მსოფლიო"], "გამარჯობა მსოფლიო"),
        (&["ሰላም", "ዓለም"], "ሰላም ዓለም"),
    ];
    for (parts, expected) in cases {
        assert_eq!(join(parts), expected);
    }
}

#[test]
fn accented_words_are_joined_with_a_space() {
    assert_eq!(join(&["café", "élégant"],), "café élégant");
    assert_eq!(join(&["está", "ótimo"]), "está ótimo");
    assert_eq!(join(&["Zażółć", "gęślą", "jaźń"]), "Zażółć gęślą jaźń");
    assert_eq!(join(&["tiếng", "Việt"]), "tiếng Việt");
}

#[test]
fn a_word_ending_in_a_combining_mark_is_still_followed_by_a_space() {
    // The same words as above in decomposed form: base letter plus combining accent.
    assert_eq!(join(&["cafe\u{0301}", "noir"]), "cafe\u{0301} noir");
    assert_eq!(join(&["esta\u{0301}", "bem"]), "esta\u{0301} bem");
    assert_eq!(join(&["приве\u{0301}", "мир"]), "приве\u{0301} мир");
}

#[test]
fn scripts_written_without_spaces_are_joined_directly() {
    assert_eq!(join(&["今天", "天气", "很好"]), "今天天气很好");
    assert_eq!(
        join(&["今日は", "いい", "天気", "ですね"]),
        "今日はいい天気ですね"
    );
    assert_eq!(join(&["コーヒー", "を", "ください"]), "コーヒーをください");
}

#[test]
fn phrases_of_scripts_that_space_only_phrases_are_joined_with_a_space() {
    // These scripts space phrases, not words; segments cut at pauses are phrases.
    assert_eq!(join(&["สวัสดีครับ", "วันนี้อากาศดี"]), "สวัสดีครับ วันนี้อากาศดี");
    assert_eq!(join(&["ສະບາຍດີ", "ມື້ນີ້"]), "ສະບາຍດີ ມື້ນີ້");
    assert_eq!(join(&["សួស្តី", "អ្នក"]), "សួស្តី អ្នក");
    assert_eq!(join(&["မင်္ဂလာပါ", "နေကောင်းလား"]), "မင်္ဂလာပါ နေကောင်းလား");
    assert_eq!(join(&["བཀྲ་ཤིས་བདེ་ལེགས།", "ཁྱེད་རང་"]), "བཀྲ་ཤིས་བདེ་ལེགས། ཁྱེད་རང་");
    assert_eq!(join(&["ผมใช้", "email", "ทุกวัน"]), "ผมใช้ email ทุกวัน");
    // Balinese, a script without data of its own.
    assert_eq!(join(&["ᬒᬁᬲ᭄ᬯᬲ᭄ᬢ᭄ᬬᬲ᭄ᬢᬸ", "ᬫᬗ᭄ᬕᬾᬄ"]), "ᬒᬁᬲ᭄ᬯᬲ᭄ᬢ᭄ᬬᬲ᭄ᬢᬸ ᬫᬗ᭄ᬕᬾᬄ");
}

#[test]
fn unspaced_scripts_stay_joined_across_their_own_punctuation() {
    assert_eq!(
        join(&["今天天气很好。", "我们出去吧"]),
        "今天天气很好。我们出去吧"
    );
    assert_eq!(
        join(&["こんにちは、", "元気ですか"]),
        "こんにちは、元気ですか"
    );
}

#[test]
fn whitespace_the_engine_supplied_is_not_doubled() {
    assert_eq!(join(&["hello ", "world"]), "hello world");
    assert_eq!(join(&["hello", " world"]), "hello world");
    assert_eq!(join(&["привет ", "мир"]), "привет мир");
    assert_eq!(join(&["안녕하세요", " 여러분"]), "안녕하세요 여러분");
    assert_eq!(join(&["bom", " dia", " a", " todos"]), "bom dia a todos");
    assert_eq!(join(&["line one\n", "line two"]), "line one\nline two");
}

#[test]
fn whitespace_on_both_sides_of_a_boundary_is_kept_as_the_engine_gave_it() {
    assert_eq!(join(&["hello ", " world"]), "hello  world");
}

#[test]
fn sentence_punctuation_followed_by_a_word_gets_one_space() {
    assert_eq!(join(&["Hello.", "World"]), "Hello. World");
    assert_eq!(
        join(&["Bonjour,", "tout le monde"]),
        "Bonjour, tout le monde"
    );
    assert_eq!(join(&["Sério?", "Sim!", "Claro."]), "Sério? Sim! Claro.");
    assert_eq!(join(&["Привет!", "Как дела?"]), "Привет! Как дела?");
    assert_eq!(join(&["Note:", "важно;", "очень"]), "Note: важно; очень");
    assert_eq!(
        join(&["안녕하세요.", "반갑습니다"]),
        "안녕하세요. 반갑습니다"
    );
}

#[test]
fn sentence_punctuation_already_followed_by_a_space_gets_no_second_one() {
    assert_eq!(join(&["Hello. ", "World"]), "Hello. World");
    assert_eq!(join(&["Hello.", " World"]), "Hello. World");
}

#[test]
fn a_sentence_followed_by_one_that_opens_with_an_inverted_mark_gets_one_space() {
    // Spanish questions and exclamations open with an inverted mark before the first word.
    assert_eq!(join(&["Hola.", "¿Cómo estás?"]), "Hola. ¿Cómo estás?");
    assert_eq!(join(&["Vale", "¡vamos!"]), "Vale ¡vamos!");
}

#[test]
fn no_space_goes_next_to_a_script_written_without_any_spaces() {
    assert_eq!(join(&["我们用", "Lumora"]), "我们用Lumora");
    assert_eq!(join(&["Lumora", "很好用"]), "Lumora很好用");
    assert_eq!(join(&["会議は", "Monday", "です"]), "会議はMondayです");
    assert_eq!(join(&["Привет", "世界"]), "Привет世界");
    assert_eq!(join(&["안녕", "世界"]), "안녕世界");
}

#[test]
fn numbers_are_spaced_like_words_except_next_to_scripts_without_spaces() {
    assert_eq!(join(&["room", "42"]), "room 42");
    assert_eq!(join(&["42", "rooms"]), "42 rooms");
    assert_eq!(join(&["комната", "42"]), "комната 42");
    assert_eq!(join(&["第", "3", "章"]), "第3章");
    assert_eq!(join(&["ห้อง", "42"]), "ห้อง 42");
}

#[test]
fn empty_segments_add_nothing() {
    assert_eq!(join(&[]), "");
    assert_eq!(join(&[""]), "");
    assert_eq!(join(&["", "hello", "", "world", ""]), "hello world");
    assert_eq!(join(&["今天", "", "天气"]), "今天天气");
}

#[test]
fn a_single_segment_is_returned_unchanged() {
    for text in ["  spaced out  ", "Olá, mundo!", "今天天气很好", "আমি আজ যাব"]
    {
        assert_eq!(join(&[text]), text);
    }
}

#[test]
fn the_text_inside_a_segment_is_never_altered() {
    assert_eq!(join(&["a  b", "c   d"]), "a  b c   d");
    assert_eq!(join(&["你好 世界", "再见"]), "你好 世界再见");
}
