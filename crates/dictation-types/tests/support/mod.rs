#![allow(dead_code)]

use dictation_types::{
    AsrCaps, Language, LanguageMixing, Languages, Transcript, TranscriptId, TranscriptSegment,
};

pub fn lang(tag: &str) -> Language {
    Language::parse(tag).unwrap_or_else(|e| panic!("'{tag}' should be a valid tag: {e}"))
}

pub fn langs(spec: &str) -> Languages {
    Languages::parse(spec).unwrap_or_else(|e| panic!("'{spec}' should be a valid list: {e}"))
}

pub fn tags(list: &[&str]) -> Vec<Language> {
    list.iter().map(|tag| lang(tag)).collect()
}

/// A batch engine for `supported` with the given mixing ability.
pub fn engine(supported: &[&str], mixing: LanguageMixing) -> AsrCaps {
    let mut caps = AsrCaps::new(tags(supported), 16_000, true);
    caps.language_mixing = mixing;
    caps
}

pub fn one_per_utterance(supported: &[&str]) -> AsrCaps {
    engine(supported, LanguageMixing::OnePerUtterance)
}

pub fn mixing_up_to(supported: &[&str], max: Option<usize>) -> AsrCaps {
    engine(supported, LanguageMixing::Mixed { max_languages: max })
}

pub fn seg(text: &str) -> TranscriptSegment {
    TranscriptSegment::new(text, 0, 0).unwrap()
}

pub fn transcript(text: &str, spec: &str) -> Transcript {
    Transcript::new(TranscriptId::new("t-1"), text, langs(spec))
}

pub fn from_segments(segments: Vec<TranscriptSegment>, spec: &str) -> Transcript {
    Transcript::from_segments(TranscriptId::new("t-1"), segments, langs(spec))
}
