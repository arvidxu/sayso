mod support;

use dictation_types::{HistoryEntry, HistoryId, ModeId, Transcript};
use support::transcript;

fn entry(transcript: Transcript) -> HistoryEntry {
    HistoryEntry::new(
        HistoryId::new("h-1"),
        transcript,
        ModeId::new("mode-x"),
        1_000,
    )
}

#[test]
fn history_counts_the_characters_of_the_raw_text_before_processing() {
    assert_eq!(entry(transcript("hello", "en")).character_count, 5);
}

#[test]
fn history_counts_the_characters_of_the_processed_text_once_there_is_one() {
    let processed = transcript("um, olá a todos", "pt").with_final_text("Olá.");
    assert_eq!(entry(processed).character_count, 4);
}

#[test]
fn history_counts_nothing_when_processing_removed_everything() {
    let emptied = transcript("um uh er", "en+fr").with_final_text("");
    let saved = entry(emptied);
    assert_eq!(saved.character_count, 0);
    assert_eq!(saved.transcript.effective_text(), "");
}

#[test]
fn history_counts_characters_not_bytes_in_every_script() {
    for (text, count) in [
        ("olá", 3),
        ("Привет", 6),
        ("안녕하세요", 5),
        ("今天天气很好", 6),
        ("বাংলা", 5),
        ("สวัสดี", 6),
        ("שלום", 4),
    ] {
        assert_eq!(
            entry(transcript(text, "en+fr")).character_count,
            count,
            "{text}"
        );
        assert_eq!(text.chars().count(), count);
    }
}

#[test]
fn a_new_history_entry_has_no_recording_until_one_is_attached() {
    let saved = entry(transcript("olá", "pt"));
    assert_eq!(saved.audio_path, None);
    assert_eq!(saved.mode_id, ModeId::new("mode-x"));
    assert_eq!(saved.created_at_ms, 1_000);
    let with_audio = saved.with_audio_path("recordings/h-1.wav");
    assert_eq!(with_audio.audio_path.as_deref(), Some("recordings/h-1.wav"));
}
