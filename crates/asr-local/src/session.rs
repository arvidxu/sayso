use std::sync::Arc;

use dictation_api::AsrSession;
use dictation_types::{AppError, AppResult, AsrOptions, Transcript};

use crate::engine::Inner;

pub(crate) struct BufferedSession {
    engine: Arc<Inner>,
    opts: AsrOptions,
    samples: Vec<f32>,
    ended: bool,
}

impl BufferedSession {
    pub(crate) fn new(engine: Arc<Inner>, opts: AsrOptions) -> Self {
        Self {
            engine,
            opts,
            samples: Vec::new(),
            ended: false,
        }
    }

    fn ensure_open(&self) -> AppResult<()> {
        if self.ended {
            return Err(AppError::asr("the recognition session has already ended"));
        }
        Ok(())
    }
}

impl AsrSession for BufferedSession {
    fn push_audio(&mut self, samples: &[f32]) -> AppResult<Option<Transcript>> {
        self.ensure_open()?;
        self.engine
            .check_length(self.samples.len().saturating_add(samples.len()))?;
        self.samples.extend_from_slice(samples);
        Ok(None)
    }

    /// On failure the session keeps its audio and stays open, so finishing can be retried.
    fn finish(&mut self) -> AppResult<Transcript> {
        self.ensure_open()?;
        let transcript = self.engine.recognise(&self.samples, &self.opts)?;
        self.ended = true;
        self.samples = Vec::new();
        Ok(transcript)
    }

    fn cancel(&mut self) -> AppResult<()> {
        self.ended = true;
        self.samples = Vec::new();
        Ok(())
    }
}
