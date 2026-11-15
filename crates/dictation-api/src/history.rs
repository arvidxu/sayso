use dictation_types::{AppResult, HistoryEntry, HistoryId, HistoryQuery, HistoryStats};

pub trait HistoryStore: Send + Sync {
    /// Replaces an entry with the same ID.
    fn append(&self, entry: HistoryEntry) -> AppResult<()>;

    fn get(&self, id: &HistoryId) -> AppResult<Option<HistoryEntry>>;

    /// Entries matching `query`, newest first. Text is matched in whatever language it is
    /// written, without regard to letter case.
    fn search(&self, query: &HistoryQuery) -> AppResult<Vec<HistoryEntry>>;

    fn delete(&self, id: &HistoryId) -> AppResult<bool>;

    fn clear(&self) -> AppResult<()>;

    fn stats(&self) -> AppResult<HistoryStats>;

    /// Forgets the recordings of entries created before `cutoff_timestamp_ms`, keeping their
    /// text. Returns how many recordings were dropped.
    fn prune_expired_audio(&self, cutoff_timestamp_ms: u64) -> AppResult<usize>;
}
