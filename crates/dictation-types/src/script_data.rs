//! Scripts that put no space between words; unlisted scripts are treated as spaced.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Spacing {
    /// Every unlisted script.
    BetweenWords,
    /// Spaces between phrases only; pieces cut at pauses are phrases.
    BetweenPhrases,
    None,
    /// Shared with spaced scripts, such as CJK brackets: the neighbouring letters decide.
    OfNeighbours,
}

/// Ascending ranges of unspaced scripts, with the punctuation blocks only they use.
const SCRIPTS: &[(char, char, Spacing)] = &[
    // Thai and Lao.
    ('\u{0E00}', '\u{0EFF}', Spacing::BetweenPhrases),
    // Tibetan.
    ('\u{0F00}', '\u{0FFF}', Spacing::BetweenPhrases),
    // Myanmar.
    ('\u{1000}', '\u{109F}', Spacing::BetweenPhrases),
    // Khmer.
    ('\u{1780}', '\u{17FF}', Spacing::BetweenPhrases),
    // Khmer symbols.
    ('\u{19E0}', '\u{19FF}', Spacing::BetweenPhrases),
    // Balinese.
    ('\u{1B00}', '\u{1B7F}', Spacing::BetweenPhrases),
    // CJK radicals, Kangxi radicals and ideographic description characters.
    ('\u{2E80}', '\u{2FFF}', Spacing::None),
    // CJK symbols and punctuation, which Korean shares.
    ('\u{3000}', '\u{303F}', Spacing::OfNeighbours),
    // Hiragana and Katakana.
    ('\u{3040}', '\u{30FF}', Spacing::None),
    // Bopomofo.
    ('\u{3100}', '\u{312F}', Spacing::None),
    // Kanbun, Bopomofo extended, CJK strokes and Katakana phonetic extensions.
    ('\u{3190}', '\u{31FF}', Spacing::None),
    // Enclosed CJK letters and months, which hold enclosed Hangul as well.
    ('\u{3200}', '\u{32FF}', Spacing::OfNeighbours),
    // CJK compatibility and CJK ideographs extension A.
    ('\u{3300}', '\u{4DBF}', Spacing::None),
    // CJK ideographs.
    ('\u{4E00}', '\u{9FFF}', Spacing::None),
    // Yi syllables and radicals.
    ('\u{A000}', '\u{A4CF}', Spacing::None),
    // Javanese and Myanmar extended B.
    ('\u{A980}', '\u{A9FF}', Spacing::BetweenPhrases),
    // Myanmar extended A.
    ('\u{AA60}', '\u{AA7F}', Spacing::BetweenPhrases),
    // CJK compatibility ideographs.
    ('\u{F900}', '\u{FAFF}', Spacing::None),
    // Vertical forms, CJK compatibility forms and small form variants.
    ('\u{FE10}', '\u{FE1F}', Spacing::None),
    ('\u{FE30}', '\u{FE6F}', Spacing::None),
    // Fullwidth forms and halfwidth Katakana (not halfwidth Hangul, which is spaced).
    ('\u{FF00}', '\u{FF9F}', Spacing::None),
    ('\u{FFE0}', '\u{FFEF}', Spacing::None),
    // Ideographic symbols, Tangut, Khitan and the Tangut supplement.
    ('\u{16FE0}', '\u{18D7F}', Spacing::None),
    // Kana extended B, kana supplement, kana extended A, small kana extension and Nüshu.
    ('\u{1AFF0}', '\u{1B2FF}', Spacing::None),
    // CJK ideographs of the supplementary planes.
    ('\u{20000}', '\u{3FFFD}', Spacing::None),
];

/// Unlisted scripts and script-less characters count as spaced; letters in shared blocks,
/// such as `々`, as unspaced.
pub(crate) fn spacing(ch: char) -> Spacing {
    let listed = SCRIPTS
        .iter()
        .find(|(first, last, _)| (*first..=*last).contains(&ch))
        .map_or(Spacing::BetweenWords, |(_, _, spacing)| *spacing);
    if listed == Spacing::OfNeighbours && ch.is_alphabetic() {
        Spacing::None
    } else {
        listed
    }
}
