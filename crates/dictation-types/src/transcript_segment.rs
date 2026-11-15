use crate::{AppError, AppResult, Language};

/// Language and confidence are set when the engine reports them.
#[derive(Debug, Clone, PartialEq)]
pub struct TranscriptSegment {
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub language: Option<Language>,
    pub confidence: Option<f32>,
    pub is_final: bool,
}

impl TranscriptSegment {
    /// Creates a final segment. Fails when it ends before it starts.
    pub fn new(text: impl Into<String>, start_ms: u64, end_ms: u64) -> AppResult<Self> {
        if end_ms < start_ms {
            return Err(AppError::validation(
                "segment end_ms must be greater than or equal to start_ms",
            ));
        }
        Ok(Self {
            text: text.into(),
            start_ms,
            end_ms,
            language: None,
            confidence: None,
            is_final: true,
        })
    }

    pub fn with_language(mut self, language: Language) -> Self {
        self.language = Some(language);
        self
    }

    /// Sets the confidence, kept between 0 and 1. A value that is not a number clears it.
    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = (!confidence.is_nan()).then(|| confidence.clamp(0.0, 1.0));
        self
    }

    pub fn with_final(mut self, is_final: bool) -> Self {
        self.is_final = is_final;
        self
    }

    pub fn duration_ms(&self) -> u64 {
        self.end_ms.saturating_sub(self.start_ms)
    }
}
