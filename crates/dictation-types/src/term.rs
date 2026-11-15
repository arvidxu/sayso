use std::iter::once;

use crate::{AppError, AppResult, Language, RuleId, TermId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TermKind {
    #[default]
    Word,
    /// Spelling and casing must be kept exactly.
    ExactSpelling,
    ProperNoun,
}

/// Aliases are written back as the canonical spelling.
#[derive(Debug, Clone, PartialEq)]
pub struct Term {
    pub id: TermId,
    pub canonical: String,
    pub aliases: Vec<String>,
    pub phonemes: Option<String>,
    pub kind: TermKind,
    pub language: Language,
    /// `None` leaves it to configuration.
    pub weight: Option<f32>,
    pub enabled: bool,
}

impl Term {
    /// Creates an enabled term. Fails when `canonical` is blank.
    pub fn new(
        id: TermId,
        canonical: impl Into<String>,
        kind: TermKind,
        language: Language,
    ) -> AppResult<Self> {
        let canonical = canonical.into();
        if canonical.trim().is_empty() {
            return Err(AppError::validation("term text must not be empty"));
        }
        Ok(Self {
            id,
            canonical,
            aliases: Vec::new(),
            phonemes: None,
            kind,
            language,
            weight: None,
            enabled: true,
        })
    }

    pub fn with_aliases<I, S>(mut self, aliases: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        // A blank alias would match blank text.
        self.aliases = aliases
            .into_iter()
            .map(Into::into)
            .filter(|alias: &String| !alias.trim().is_empty())
            .collect();
        self
    }

    pub fn with_phonemes(mut self, phonemes: impl Into<String>) -> Self {
        self.phonemes = Some(phonemes.into());
        self
    }

    /// Sets the weight; negative values become zero and a value that is not a number clears it.
    pub fn with_weight(mut self, weight: f32) -> Self {
        self.weight = (!weight.is_nan()).then(|| weight.max(0.0));
        self
    }

    /// Matches ignoring case (see [`fold_case`]) and surrounding whitespace.
    pub fn restore_spelling(&self, candidate: &str) -> Option<&str> {
        if !self.enabled {
            return None;
        }
        let wanted = fold_case(candidate.trim());
        let is_match = once(&self.canonical)
            .chain(self.aliases.iter())
            .any(|known| fold_case(known.trim()) == wanted);
        is_match.then_some(self.canonical.as_str())
    }
}

/// Not locale-specific (`İ` and `ı` both fold to `i`). Does not normalize composed and
/// decomposed forms; do that before calling.
pub fn fold_case(text: &str) -> String {
    text.chars()
        .map(|ch| if ch == '\u{0130}' { 'i' } else { ch })
        .collect::<String>()
        .to_lowercase()
        .to_uppercase()
        .to_lowercase()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ReplacementKind {
    #[default]
    Exact,
    CaseInsensitive,
    Regex,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementRule {
    pub id: RuleId,
    pub pattern: String,
    pub replacement: String,
    pub kind: ReplacementKind,
    pub enabled: bool,
}

impl ReplacementRule {
    /// Creates an enabled rule. Fails when `pattern` is empty.
    pub fn new(
        id: RuleId,
        pattern: impl Into<String>,
        replacement: impl Into<String>,
        kind: ReplacementKind,
    ) -> AppResult<Self> {
        let pattern = pattern.into();
        if pattern.is_empty() {
            return Err(AppError::validation(
                "replacement rule pattern must not be empty",
            ));
        }
        Ok(Self {
            id,
            pattern,
            replacement: replacement.into(),
            kind,
            enabled: true,
        })
    }
}

/// Each (word, language) pair is its own entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FillerWord {
    pub word: String,
    pub language: Language,
    pub is_builtin: bool,
}

impl FillerWord {
    pub fn new(word: impl Into<String>, language: Language, is_builtin: bool) -> Self {
        Self {
            word: word.into(),
            language,
            is_builtin,
        }
    }
}
