use dictation_types::{AppResult, AsrCaps, AsrOptions, ProviderId, Transcript};

use crate::{AsrEngine, AsrSession};

#[derive(Debug, Clone)]
pub struct FakeAsrEngine {
    provider_id: ProviderId,
    caps: AsrCaps,
    final_transcript: Transcript,
}

impl FakeAsrEngine {
    /// Answers every session with `final_transcript`.
    pub fn new(provider_id: ProviderId, caps: AsrCaps, final_transcript: Transcript) -> Self {
        Self {
            provider_id,
            caps,
            final_transcript,
        }
    }
}

struct FakeAsrSession {
    final_transcript: Transcript,
}

impl AsrSession for FakeAsrSession {
    fn push_audio(&mut self, _samples: &[f32]) -> AppResult<Option<Transcript>> {
        Ok(None)
    }

    fn finish(&mut self) -> AppResult<Transcript> {
        let mut transcript = self.final_transcript.clone();
        transcript.is_partial = false;
        Ok(transcript)
    }
}

impl AsrEngine for FakeAsrEngine {
    fn id(&self) -> ProviderId {
        self.provider_id.clone()
    }

    fn capabilities(&self) -> AsrCaps {
        self.caps.clone()
    }

    fn start(&self, _opts: AsrOptions) -> AppResult<Box<dyn AsrSession>> {
        Ok(Box::new(FakeAsrSession {
            final_transcript: self.final_transcript.clone(),
        }))
    }
}
