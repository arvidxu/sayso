//! A speech scorer backed by a small neural voice activity model that runs on the device.

use dictation_types::{AppError, AppResult};
use earshot::Detector;

use crate::voice_detection::FrameScorer;

/// Fixed by how the model was trained, not choices.
const MODEL_SAMPLE_RATE_HZ: u32 = 16_000;
const MODEL_FRAME_LEN: usize = 256;

/// The model is not tied to a language.
pub struct NeuralSpeechScorer {
    detector: Box<Detector>,
}

impl NeuralSpeechScorer {
    pub fn new() -> Self {
        Self {
            detector: Detector::default_boxed(),
        }
    }
}

impl Default for NeuralSpeechScorer {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameScorer for NeuralSpeechScorer {
    fn sample_rate_hz(&self) -> u32 {
        MODEL_SAMPLE_RATE_HZ
    }

    fn frame_len(&self) -> usize {
        MODEL_FRAME_LEN
    }

    fn score(&mut self, frame: &[f32]) -> AppResult<f32> {
        if frame.len() != MODEL_FRAME_LEN {
            return Err(AppError::validation(format!(
                "a frame for the speech scorer must have {MODEL_FRAME_LEN} samples, got {}",
                frame.len()
            )));
        }
        let score = self.detector.predict_f32(frame);
        Ok(if score.is_nan() {
            0.0
        } else {
            score.clamp(0.0, 1.0)
        })
    }

    fn reset(&mut self) {
        self.detector.reset();
    }
}
