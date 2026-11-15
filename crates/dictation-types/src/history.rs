use crate::{HistoryId, ModeId, Transcript};

#[derive(Debug, Clone, PartialEq)]
pub struct HistoryEntry {
    pub id: HistoryId,
    pub transcript: Transcript,
    pub mode_id: ModeId,
    pub created_at_ms: u64,
    pub audio_path: Option<String>,
    /// The number of characters of the text that was used.
    pub character_count: usize,
}

impl HistoryEntry {
    pub fn new(id: HistoryId, transcript: Transcript, mode_id: ModeId, created_at_ms: u64) -> Self {
        let character_count = transcript.effective_text().chars().count();
        Self {
            id,
            transcript,
            mode_id,
            created_at_ms,
            audio_path: None,
            character_count,
        }
    }

    pub fn with_audio_path(mut self, audio_path: impl Into<String>) -> Self {
        self.audio_path = Some(audio_path.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HistoryQuery {
    pub search_text: Option<String>,
    pub mode_id: Option<ModeId>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HistoryStats {
    pub total_entries: usize,
    pub total_audio_duration_ms: u64,
    pub total_characters: usize,
}
