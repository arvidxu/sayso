//! A character added to a list here must behave as the list says in every language that
//! writes it.

/// End a clause: attach to the word before, and a space follows in spaced scripts.
#[rustfmt::skip]
const CLAUSE_ENDINGS: &[char] = &[
    '.', ',', '!', '?', ';', ':',
    // Ellipsis.
    '\u{2026}',
    // Greek question mark and ano teleia.
    '\u{037E}', '\u{0387}',
    // Armenian full stop and comma.
    '\u{0589}', '\u{055D}',
    // Arabic comma, semicolon and question mark, and Urdu full stop.
    '\u{060C}', '\u{061B}', '\u{061F}', '\u{06D4}',
    // Danda and double danda, used by Indic scripts.
    '\u{0964}', '\u{0965}',
    // Tibetan shad and nyis shad.
    '\u{0F0D}', '\u{0F0E}',
    // Myanmar little and full section marks.
    '\u{104A}', '\u{104B}',
    // Ethiopic full stop, comma, semicolon, colon and question mark.
    '\u{1362}', '\u{1363}', '\u{1364}', '\u{1365}', '\u{1367}',
    // Khmer khan and bariyoosan.
    '\u{17D4}', '\u{17D5}',
    // Balinese carik and carik pareren.
    '\u{1B5E}', '\u{1B5F}',
    // Ideographic comma and full stop.
    '\u{3001}', '\u{3002}',
    // Javanese lingsa and lungsi.
    '\u{A9C8}', '\u{A9C9}',
];

/// Nothing goes before or after them: the Tibetan tsheg and its non-breaking form.
const SYLLABLE_SEPARATORS: &[char] = &['\u{0F0B}', '\u{0F0C}'];

/// Only ever open, so no space follows them.
const OPENERS: &[char] = &[
    '(', '[', '{', '\u{00BF}', '\u{00A1}', '\u{3008}', '\u{300A}', '\u{300C}', '\u{300E}',
    '\u{3010}', '\u{3014}', '\u{3016}', '\u{3018}', '\u{301A}',
];

/// Only ever close, so no space precedes them; same order as [`OPENERS`].
const CLOSERS: &[char] = &[
    ')', ']', '}', '\u{3009}', '\u{300B}', '\u{300D}', '\u{300F}', '\u{3011}', '\u{3015}',
    '\u{3017}', '\u{3019}', '\u{301B}',
];

/// (opening, closing). One character may open one quote and close another; the quote open
/// before it, or whether it starts a word, decides.
const QUOTE_PAIRS: &[(char, char)] = &[
    ('"', '"'),
    ('\'', '\''),
    ('\u{00AB}', '\u{00BB}'),
    ('\u{00BB}', '\u{00AB}'),
    ('\u{00BB}', '\u{00BB}'),
    ('\u{2039}', '\u{203A}'),
    ('\u{203A}', '\u{2039}'),
    ('\u{203A}', '\u{203A}'),
    ('\u{201C}', '\u{201D}'),
    ('\u{201D}', '\u{201D}'),
    ('\u{201E}', '\u{201C}'),
    ('\u{201E}', '\u{201D}'),
    ('\u{2018}', '\u{2019}'),
    ('\u{2019}', '\u{2019}'),
    ('\u{201A}', '\u{2018}'),
    ('\u{201A}', '\u{2019}'),
];

/// Also written as apostrophes inside a word, where they open or close nothing.
const APOSTROPHES: &[char] = &['\'', '\u{2019}'];

pub(crate) fn is_clause_ending(ch: char) -> bool {
    CLAUSE_ENDINGS.contains(&ch)
}

pub(crate) fn is_syllable_separator(ch: char) -> bool {
    SYLLABLE_SEPARATORS.contains(&ch)
}

pub(crate) fn is_opener(ch: char) -> bool {
    OPENERS.contains(&ch)
}

pub(crate) fn is_closer(ch: char) -> bool {
    CLOSERS.contains(&ch)
}

pub(crate) fn opens_quote(ch: char) -> bool {
    QUOTE_PAIRS.iter().any(|(open, _)| *open == ch)
}

/// Its shape says it opens, like `“` or `«`; `false` for `”` and `"`, which close themselves.
pub(crate) fn opens_with_distinct_closer(ch: char) -> bool {
    QUOTE_PAIRS
        .iter()
        .any(|(open, close)| *open == ch && *close != ch)
}

pub(crate) fn is_quote(ch: char) -> bool {
    QUOTE_PAIRS
        .iter()
        .any(|(open, close)| *open == ch || *close == ch)
}

pub(crate) fn closes_quote(open: char, close: char) -> bool {
    QUOTE_PAIRS.contains(&(open, close))
}

pub(crate) fn is_apostrophe(ch: char) -> bool {
    APOSTROPHES.contains(&ch)
}
