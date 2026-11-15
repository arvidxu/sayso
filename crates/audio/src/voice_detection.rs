use std::collections::VecDeque;
use std::fmt;

use dictation_types::{AppError, AppResult, AudioChunk};

use crate::measure::{loudness, ms_to_frames};
use crate::speech_span::SpeechSpan;
use crate::{ResampleTuning, resample_to_mono};

/// Says how likely a short frame is to be speech, at the format it declares.
pub trait FrameScorer: Send {
    fn sample_rate_hz(&self) -> u32;

    fn frame_len(&self) -> usize;

    /// From 0 to 1 for a frame of samples within full scale. Frames arrive in order, so a
    /// scorer may carry state between them.
    fn score(&mut self, frame: &[f32]) -> AppResult<f32>;

    fn reset(&mut self);
}

#[derive(Debug, Clone, PartialEq)]
pub struct VoiceDetectionConfig {
    pub speech_threshold: f32,
    /// Not above `speech_threshold`.
    pub silence_threshold: f32,
    /// Root mean square, 0 to 1; quieter frames are never speech.
    pub min_loudness: f32,
    /// Quiet audio is boosted before scoring so the loudest recent frame reaches this, which
    /// helps distant speech. Never turned down; 0 disables.
    pub level_to: f32,
    pub level_window_ms: u32,
    pub min_speech_ms: u32,
    pub min_silence_ms: u32,
    pub pad_ms: u32,
}

impl VoiceDetectionConfig {
    pub fn validate(&self) -> AppResult<()> {
        let in_unit = |value: f32| (0.0..=1.0).contains(&value);
        if !(self.speech_threshold > 0.0 && self.speech_threshold <= 1.0) {
            return Err(AppError::validation(
                "speech_threshold must be above 0, or every sound counts as speech, and at most 1",
            ));
        }
        if !in_unit(self.silence_threshold) || self.silence_threshold > self.speech_threshold {
            return Err(AppError::validation(
                "silence_threshold must be between 0 and speech_threshold",
            ));
        }
        if !in_unit(self.min_loudness) {
            return Err(AppError::validation("min_loudness must be between 0 and 1"));
        }
        if !in_unit(self.level_to) {
            return Err(AppError::validation("level_to must be between 0 and 1"));
        }
        if self.level_to > 0.0 && self.level_window_ms == 0 {
            return Err(AppError::validation(
                "level_window_ms must be greater than zero when level_to is used",
            ));
        }
        Ok(())
    }
}

pub struct VoiceDetector<S: FrameScorer> {
    config: VoiceDetectionConfig,
    resampling: ResampleTuning,
    scorer: S,
}

impl<S: FrameScorer> fmt::Debug for VoiceDetector<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VoiceDetector")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl<S: FrameScorer> VoiceDetector<S> {
    pub fn new(
        config: VoiceDetectionConfig,
        scorer: S,
        resampling: ResampleTuning,
    ) -> AppResult<Self> {
        config.validate()?;
        resampling.validate()?;
        if scorer.frame_len() == 0 || scorer.sample_rate_hz() == 0 {
            return Err(AppError::validation(
                "a frame scorer must declare a frame length and a sample rate",
            ));
        }
        Ok(Self {
            config,
            resampling,
            scorer,
        })
    }

    /// `samples` must be mono at the scorer's rate and within full scale, as resampling leaves
    /// them. `None` marks a frame below the loudness floor; a trailing partial frame is padded
    /// with silence.
    fn frame_scores(&mut self, samples: &[f32]) -> AppResult<Vec<Option<f32>>> {
        self.scorer.reset();
        let frame_len = self.scorer.frame_len();
        let mut scores = Vec::with_capacity(samples.len().div_ceil(frame_len));
        let mut frame = vec![0.0f32; frame_len];
        let window_samples = ms_to_frames(
            u64::from(self.config.level_window_ms),
            self.scorer.sample_rate_hz(),
        );
        let window = (window_samples / frame_len.max(1)).max(1);
        let mut recent: VecDeque<f32> = VecDeque::with_capacity(window);
        for piece in samples.chunks(frame_len) {
            frame.fill(0.0);
            frame[..piece.len()].copy_from_slice(piece);
            // Every frame is scored to keep the scorer's state true to the audio. Loudness
            // excludes the padding of a short last frame.
            let level = loudness(&frame[..piece.len()]);
            if recent.len() == window {
                recent.pop_front();
            }
            recent.push_back(level);
            let loudest = recent.iter().copied().fold(0.0f32, f32::max);
            if self.config.level_to > loudest
                && loudest > 0.0
                && loudest >= self.config.min_loudness
            {
                let gain = self.config.level_to / loudest;
                for sample in &mut frame {
                    *sample = (*sample * gain).clamp(-1.0, 1.0);
                }
            }
            let score = self.scorer.score(&frame)?;
            scores.push((level >= self.config.min_loudness).then_some(score));
        }
        Ok(scores)
    }

    /// Spans are in frames of `chunk`, in order, and do not overlap.
    pub fn detect(&mut self, chunk: &AudioChunk) -> AppResult<Vec<SpeechSpan>> {
        if chunk.channels == 0 {
            return Err(AppError::validation(
                "audio channel count must be greater than zero",
            ));
        }
        let input_frames = chunk.samples.len() / usize::from(chunk.channels);
        if input_frames == 0 {
            return Ok(Vec::new());
        }
        let rate = self.scorer.sample_rate_hz();
        let analysed = resample_to_mono(chunk, rate, &self.resampling)?;
        let total = analysed.samples.len();
        let frame_len = self.scorer.frame_len();
        let scores = self.frame_scores(&analysed.samples)?;

        let min_speech = ms_to_frames(u64::from(self.config.min_speech_ms), rate);
        let min_silence = ms_to_frames(u64::from(self.config.min_silence_ms), rate);
        let pad = ms_to_frames(u64::from(self.config.pad_ms), rate);

        // (start, end) in samples at the scorer rate.
        let mut found: Vec<(usize, usize)> = Vec::new();
        let mut open: Option<(usize, usize)> = None;
        let speech = self.config.speech_threshold;
        let silence = self.config.silence_threshold;
        for (index, scored) in scores.iter().copied().enumerate() {
            let frame_start = index * frame_len;
            let frame_end = ((index + 1) * frame_len).min(total);
            match &mut open {
                None if scored.is_some_and(|score| score >= speech) => {
                    open = Some((frame_start, frame_end));
                }
                None => {}
                Some((_, end)) if scored.is_some_and(|score| score >= silence) => {
                    *end = frame_end;
                }
                Some((start, end)) if frame_end.saturating_sub(*end) >= min_silence => {
                    if end.saturating_sub(*start) >= min_speech {
                        found.push((*start, *end));
                    }
                    open = None;
                }
                // A dip that may yet turn out to be the span's end.
                Some(_) => {}
            }
        }
        if let Some((start, end)) = open
            && end.saturating_sub(start) >= min_speech
        {
            found.push((start, end));
        }

        let to_input = |sample: usize| -> usize {
            ((sample as u128 * u128::from(chunk.sample_rate_hz)) / u128::from(rate)) as usize
        };
        let mut spans: Vec<SpeechSpan> = Vec::with_capacity(found.len());
        for (start, end) in found {
            let start_frame = to_input(start.saturating_sub(pad)).min(input_frames);
            let end_frame = to_input((end + pad).min(total)).min(input_frames);
            match spans.last_mut() {
                Some(previous) if start_frame <= previous.end_frame => {
                    previous.end_frame = previous.end_frame.max(end_frame);
                }
                _ if end_frame > start_frame => spans.push(SpeechSpan {
                    start_frame,
                    end_frame,
                    sample_rate_hz: chunk.sample_rate_hz,
                }),
                _ => {}
            }
        }
        Ok(spans)
    }
}
