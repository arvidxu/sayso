use crate::{Language, TranscriptSegment};

/// Weighted by spoken time, or by letters and digits when no segment with a language is
/// timed. `None` when no segment with text has a language.
pub(crate) fn dominant_language(segments: &[TranscriptSegment]) -> Option<&Language> {
    let by_time = segments
        .iter()
        .any(|s| s.language.is_some() && s.end_ms > s.start_ms);
    let mut by_language: Vec<(&Language, usize)> = Vec::new();
    for segment in segments {
        let Some(language) = &segment.language else {
            continue;
        };
        let amount = amount_of(segment, by_time);
        match by_language.iter_mut().find(|(l, _)| l.matches(language)) {
            Some((_, total)) => *total += amount,
            None => by_language.push((language, amount)),
        }
    }
    let mut reported: Option<(&Language, usize)> = None;
    for (language, amount) in by_language {
        if amount > 0 && reported.is_none_or(|(_, top)| amount > top) {
            reported = Some((language, amount));
        }
    }
    reported.map(|(language, _)| language)
}

/// Spoken time when `by_time`, else letters and digits; zero without text.
fn amount_of(segment: &TranscriptSegment, by_time: bool) -> usize {
    let letters = segment
        .text
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .count();
    if letters == 0 {
        0
    } else if by_time {
        usize::try_from(segment.end_ms.saturating_sub(segment.start_ms)).unwrap_or(usize::MAX)
    } else {
        letters
    }
}
