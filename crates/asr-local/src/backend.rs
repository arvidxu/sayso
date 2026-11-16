use std::path::Path;

use dictation_types::{AppResult, AsrOptions, CancelToken, ModelDescriptor, ProviderId};

use crate::RecognitionTuning;

/// Dropping it frees its memory.
pub trait SpeechModel: Send + Sync {
    /// `samples` are mono at the rate the catalog entry declares and no longer than it allows.
    /// Cancelling fails at once if the model can stop mid-way, otherwise at its next chance.
    fn recognise(
        &self,
        samples: &[f32],
        sample_rate_hz: u32,
        opts: &AsrOptions,
        cancel: &CancelToken,
    ) -> AppResult<String>;

    /// A model that returns false is dropped and reloaded when next needed.
    fn is_usable(&self) -> bool {
        true
    }
}

pub trait ModelBackend: Send + Sync {
    /// The engine name catalog entries use to target this engine.
    fn engine_id(&self) -> ProviderId;

    /// Checks the catalog entry without loading the model.
    fn check(&self, model: &ModelDescriptor) -> AppResult<()>;

    /// Cancelling fails at once if the engine can stop mid-load, otherwise at its next chance.
    fn load(
        &self,
        model: &ModelDescriptor,
        dir: &Path,
        tuning: &RecognitionTuning,
        cancel: &CancelToken,
    ) -> AppResult<Box<dyn SpeechModel>>;
}
