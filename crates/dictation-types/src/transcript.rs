use crate::language_share;
use crate::{
    Language, Languages, RequestRoute, StageTimings, TokenUsage, TranscriptId, TranscriptSegment,
    join_segment_texts,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Transcript {
    pub id: TranscriptId,
    pub raw_text: String,
    /// `None` until processed; an empty string means nothing to insert.
    pub final_text: Option<String>,
    pub segments: Vec<TranscriptSegment>,
    pub languages: Languages,
    pub is_partial: bool,
    pub audio_duration_ms: u64,
    pub timings: StageTimings,
    pub request_routes: Vec<RequestRoute>,
    pub token_usage: Option<TokenUsage>,
}

impl Transcript {
    pub fn new(id: TranscriptId, raw_text: impl Into<String>, languages: Languages) -> Self {
        Self {
            id,
            raw_text: raw_text.into(),
            final_text: None,
            segments: Vec::new(),
            languages,
            is_partial: false,
            audio_duration_ms: 0,
            timings: StageTimings::default(),
            request_routes: Vec::new(),
            token_usage: None,
        }
    }

    pub fn partial(id: TranscriptId, raw_text: impl Into<String>, languages: Languages) -> Self {
        let mut transcript = Self::new(id, raw_text, languages);
        transcript.is_partial = true;
        transcript
    }

    /// Text is the segments joined in order, duration the latest end; partial while any
    /// segment is not final.
    pub fn from_segments(
        id: TranscriptId,
        segments: Vec<TranscriptSegment>,
        languages: Languages,
    ) -> Self {
        let mut transcript = Self::new(id, join_segment_texts(&segments), languages);
        transcript.audio_duration_ms = segments.iter().map(|s| s.end_ms).max().unwrap_or(0);
        transcript.is_partial = segments.iter().any(|s| !s.is_final);
        transcript.segments = segments;
        transcript
    }

    /// An empty string is kept as a result.
    pub fn with_final_text(mut self, final_text: impl Into<String>) -> Self {
        self.final_text = Some(final_text.into());
        self
    }

    pub fn with_audio_duration_ms(mut self, duration_ms: u64) -> Self {
        self.audio_duration_ms = duration_ms;
        self
    }

    pub fn with_timings(mut self, timings: StageTimings) -> Self {
        self.timings = timings;
        self
    }

    pub fn add_route(&mut self, route: RequestRoute) {
        self.request_routes.push(route);
    }

    /// The processed text once there is one, even if empty; the recognised text before that.
    pub fn effective_text(&self) -> &str {
        self.final_text.as_deref().unwrap_or(&self.raw_text)
    }

    /// Weighted by spoken time, else letters and digits; the user's own tag for it when listed.
    /// `None` when no segment with text has a language.
    pub fn dominant_language(&self) -> Option<Language> {
        language_share::dominant_language(&self.segments)
            .map(|language| self.languages.find(language).unwrap_or(language).clone())
    }

    /// Returns `true` if the engine reported more than one language across segments.
    pub fn is_code_mixed(&self) -> bool {
        let mut first: Option<&Language> = None;
        for language in self.segments.iter().filter_map(|s| s.language.as_ref()) {
            match first {
                None => first = Some(language),
                Some(seen) if !seen.matches(language) => return true,
                Some(_) => {}
            }
        }
        false
    }
}
