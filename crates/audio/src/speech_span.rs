use dictation_types::{AppError, AppResult, AudioChunk};

use crate::downmix_to_mono;
use crate::measure::frames_to_ms;

/// A stretch of a recording that contains speech, in frames of that recording.
#[derive(Debug, Clone, PartialEq)]
pub struct SpeechSpan {
    pub start_frame: usize,
    /// Exclusive.
    pub end_frame: usize,
    pub sample_rate_hz: u32,
}

impl SpeechSpan {
    pub fn frames(&self) -> usize {
        self.end_frame.saturating_sub(self.start_frame)
    }

    pub fn start_ms(&self) -> u64 {
        frames_to_ms(self.start_frame, self.sample_rate_hz)
    }

    pub fn end_ms(&self) -> u64 {
        frames_to_ms(self.end_frame, self.sample_rate_hz)
    }

    pub fn duration_ms(&self) -> u64 {
        frames_to_ms(self.frames(), self.sample_rate_hz)
    }
}

/// Spans must share the chunk's rate, lie inside it, be in order and not overlap.
pub(crate) fn check_spans(
    chunk: &AudioChunk,
    frames: usize,
    spans: &[SpeechSpan],
) -> AppResult<()> {
    let mut previous_end = 0usize;
    for span in spans {
        if span.sample_rate_hz != chunk.sample_rate_hz {
            return Err(AppError::validation(format!(
                "a speech span at {} Hz does not describe a recording at {} Hz",
                span.sample_rate_hz, chunk.sample_rate_hz
            )));
        }
        if span.end_frame < span.start_frame || span.end_frame > frames {
            return Err(AppError::validation(
                "a speech span lies outside the recording",
            ));
        }
        if span.start_frame < previous_end {
            return Err(AppError::validation(
                "speech spans must be in order and must not overlap",
            ));
        }
        previous_end = span.end_frame;
    }
    Ok(())
}

/// Returns the recording as mono with everything outside `spans` cut out.
pub fn keep_speech(chunk: &AudioChunk, spans: &[SpeechSpan]) -> AppResult<AudioChunk> {
    let mono = downmix_to_mono(&chunk.samples, chunk.channels)?;
    check_spans(chunk, mono.len(), spans)?;
    let mut kept = Vec::with_capacity(spans.iter().map(SpeechSpan::frames).sum());
    for span in spans {
        kept.extend_from_slice(&mono[span.start_frame..span.end_frame]);
    }
    Ok(AudioChunk::new(kept, chunk.sample_rate_hz, 1))
}
