use std::slice::Iter;

use std::fmt;
use std::str::FromStr;

use crate::{AppError, AppResult, Language};

/// One or more, in order of preference, the first primary; exact duplicates are dropped.
/// Written as `fr+en+ja`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Languages(Vec<Language>);

impl Languages {
    /// Drops exact duplicates; fails when empty.
    pub fn new(languages: impl IntoIterator<Item = Language>) -> AppResult<Self> {
        let mut unique: Vec<Language> = Vec::new();
        for language in languages {
            if !unique.contains(&language) {
                unique.push(language);
            }
        }
        if unique.is_empty() {
            return Err(AppError::validation("at least one language must be given"));
        }
        Ok(Self(unique))
    }

    pub fn single(language: Language) -> Self {
        Self(vec![language])
    }

    pub fn primary(&self) -> &Language {
        &self.0[0]
    }

    pub fn as_slice(&self) -> &[Language] {
        &self.0
    }

    pub fn iter(&self) -> Iter<'_, Language> {
        self.0.iter()
    }

    /// Never zero.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Always `false`: the list holds at least one language.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Counts variants of one language (`pt-BR`, `pt-PT`) once.
    pub fn distinct_languages(&self) -> usize {
        let mut seen: Vec<&str> = Vec::new();
        for language in &self.0 {
            if !seen.contains(&language.canonical_primary()) {
                seen.push(language.canonical_primary());
            }
        }
        seen.len()
    }

    /// Returns `true` if more than one distinct language is listed.
    pub fn is_multilingual(&self) -> bool {
        self.distinct_languages() > 1
    }

    /// Any variant of `language` counts.
    pub fn contains(&self, language: &Language) -> bool {
        self.0.iter().any(|l| l.matches(language))
    }

    /// The identical tag if listed, else the first variant of the same language.
    pub fn find(&self, language: &Language) -> Option<&Language> {
        self.0
            .iter()
            .find(|l| *l == language)
            .or_else(|| self.0.iter().find(|l| l.matches(language)))
    }

    /// The tags joined by `+`, for example `fr+en+ja`.
    pub fn code(&self) -> String {
        self.0
            .iter()
            .map(Language::code)
            .collect::<Vec<_>>()
            .join("+")
    }

    /// Parses tags separated by `+`, `/`, or `,`, such as `fr+en+ja`, `ko/en`, or `de`.
    pub fn parse(spec: &str) -> AppResult<Self> {
        let languages = spec
            .split(['+', '/', ','])
            .map(Language::parse)
            .collect::<AppResult<Vec<_>>>()?;
        Self::new(languages)
    }
}

impl fmt::Display for Languages {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.code())
    }
}

impl FromStr for Languages {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl From<Language> for Languages {
    fn from(language: Language) -> Self {
        Self::single(language)
    }
}

impl<'a> IntoIterator for &'a Languages {
    type Item = &'a Language;
    type IntoIter = Iter<'a, Language>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}
