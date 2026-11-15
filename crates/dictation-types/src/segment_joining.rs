use crate::TranscriptSegment;
use crate::mark_data::{is_attached_mark, is_joiner, is_regional_indicator};
use crate::punctuation_data as punctuation;
use crate::script_data::{Spacing, spacing};

/// One space between segments unless the script is unspaced or boundary punctuation attaches
/// to one side. Engine whitespace is kept; segments must be whole words.
pub fn join_segment_texts(segments: &[TranscriptSegment]) -> String {
    let mut joined = String::new();
    for segment in segments {
        if needs_space(&joined, &segment.text) {
            joined.push(' ');
        }
        joined.push_str(&segment.text);
    }
    joined
}

/// At the start, after a space, or after something that opens (`prev_opened`).
fn can_precede_opening_quote(prev: Option<char>, prev_opened: bool) -> bool {
    prev.is_none_or(|p| p.is_whitespace() || punctuation::is_opener(p) || prev_opened)
}

struct QuoteScan {
    /// Still open at the end, innermost last.
    open: Vec<char>,
    last_opened: bool,
    /// Whether every quote open at the start was closed at some point.
    closed_all: bool,
}

/// `open` holds the quotes already open before `chars`.
fn scan_quotes(chars: &[char], mut open: Vec<char>) -> QuoteScan {
    let mut last_opened = false;
    let mut closed_all = open.is_empty();
    for (i, &ch) in chars.iter().enumerate() {
        let prev_opened = last_opened;
        last_opened = false;
        if !punctuation::is_quote(ch) {
            continue;
        }
        let prev = i.checked_sub(1).map(|p| chars[p]);
        let next = chars.get(i + 1).copied();
        let apostrophe = punctuation::is_apostrophe(ch);
        if apostrophe
            && prev.is_some_and(char::is_alphanumeric)
            && next.is_some_and(char::is_alphanumeric)
        {
            continue;
        }
        if open
            .last()
            .is_some_and(|opener| punctuation::closes_quote(*opener, ch))
        {
            open.pop();
            closed_all |= open.is_empty();
        } else if punctuation::opens_quote(ch) && can_precede_opening_quote(prev, prev_opened) {
            // An apostrophe that starts a word opens a quote only when the quote is closed.
            let starts_a_word = apostrophe && next.is_some_and(char::is_alphanumeric);
            if !starts_a_word || is_closed_later(&chars[i..]) {
                open.push(ch);
                last_opened = true;
            }
        }
    }
    QuoteScan {
        open,
        last_opened,
        closed_all,
    }
}

fn is_closed_later(chars: &[char]) -> bool {
    match chars.split_first() {
        Some((&quote, rest)) => scan_quotes(rest, vec![quote]).closed_all,
        None => false,
    }
}

/// `open` holds the quotes open before `after`.
fn attaches_to_before(after: &[char], open: &[char]) -> bool {
    let Some(&ch) = after.first() else {
        return false;
    };
    if punctuation::is_clause_ending(ch)
        || punctuation::is_closer(ch)
        || punctuation::is_syllable_separator(ch)
    {
        return true;
    }
    if !punctuation::is_quote(ch) {
        return false;
    }
    if open
        .last()
        .is_some_and(|opener| punctuation::closes_quote(*opener, ch))
    {
        return true;
    }
    // The quote closes nothing open. It opens one when the segment closes it later.
    if punctuation::opens_quote(ch) && is_closed_later(after) {
        return false;
    }
    match after.get(1) {
        // Alone, it opens a quote, unless it is an apostrophe, which ends the word before.
        None => !punctuation::opens_quote(ch) || punctuation::is_apostrophe(ch),
        // Before a word, it opens a quote or starts the word.
        Some(next) if next.is_alphanumeric() => false,
        // Before a space, it opens a quote when its shape says it opens one, as `«` does.
        Some(next) if next.is_whitespace() => !punctuation::opens_with_distinct_closer(ch),
        // Before more punctuation, it ends the text before, as in `”, she said`.
        Some(_) => true,
    }
}

/// Spacing of the first character that does not take it from its neighbours;
/// [`Spacing::BetweenWords`] if none.
fn neighbouring_spacing(chars: impl Iterator<Item = char>) -> Spacing {
    chars
        .map(spacing)
        .find(|s| *s != Spacing::OfNeighbours)
        .unwrap_or(Spacing::BetweenWords)
}

fn needs_space(before: &str, after: &str) -> bool {
    let after: Vec<char> = after.chars().collect();
    let (Some(boundary_last), Some(&first)) = (before.chars().last(), after.first()) else {
        return false;
    };
    if boundary_last.is_whitespace()
        || first.is_whitespace()
        || is_attached_mark(first)
        || is_joiner(boundary_last)
    {
        return false;
    }
    let trailing_indicators = before
        .chars()
        .rev()
        .take_while(|ch| is_regional_indicator(*ch))
        .count();
    if is_regional_indicator(first) && !trailing_indicators.is_multiple_of(2) {
        return false;
    }
    let Some(last) = before.chars().rev().find(|ch| !is_attached_mark(*ch)) else {
        return false;
    };
    if punctuation::is_syllable_separator(last) {
        return false;
    }

    let unmarked_before = before.chars().rev().filter(|ch| !is_attached_mark(*ch));
    if neighbouring_spacing(unmarked_before) == Spacing::None
        || neighbouring_spacing(after.iter().copied()) == Spacing::None
    {
        return false;
    }
    let quotes = scan_quotes(&before.chars().collect::<Vec<_>>(), Vec::new());
    if punctuation::is_opener(last) || quotes.last_opened {
        return false;
    }
    !attaches_to_before(&after, &quotes.open)
}
