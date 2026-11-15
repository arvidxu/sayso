use dictation_types::{AppResult, FillerWord, Language, ReplacementRule, RuleId, Term, TermId};

/// The personal dictionary: terms, replacement rules, and filler words.
pub trait DictionaryStore: Send + Sync {
    fn upsert_term(&self, term: Term) -> AppResult<()>;

    fn get_term(&self, id: &TermId) -> AppResult<Option<Term>>;

    fn list_terms(&self) -> AppResult<Vec<Term>>;

    fn remove_term(&self, id: &TermId) -> AppResult<bool>;

    fn upsert_rule(&self, rule: ReplacementRule) -> AppResult<()>;

    fn list_rules(&self) -> AppResult<Vec<ReplacementRule>>;

    fn remove_rule(&self, id: &RuleId) -> AppResult<bool>;

    /// The same spelling in another language is a separate entry.
    fn add_filler_word(&self, word: FillerWord) -> AppResult<()>;

    fn list_filler_words(&self) -> AppResult<Vec<FillerWord>>;

    fn remove_filler_word(&self, word: &str, language: &Language) -> AppResult<bool>;
}
