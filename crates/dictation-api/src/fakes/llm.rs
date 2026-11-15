use dictation_types::{AppResult, LlmCaps, LlmChunk, LlmRequest, ProviderId};

use crate::{BoxStream, LlmEngine};

#[derive(Debug, Clone)]
pub struct FakeLlmEngine {
    provider_id: ProviderId,
    caps: LlmCaps,
    chunks: Vec<String>,
}

impl FakeLlmEngine {
    /// Answers every request with `chunks`.
    pub fn new(provider_id: ProviderId, caps: LlmCaps, chunks: Vec<String>) -> Self {
        Self {
            provider_id,
            caps,
            chunks,
        }
    }
}

impl LlmEngine for FakeLlmEngine {
    fn id(&self) -> ProviderId {
        self.provider_id.clone()
    }

    fn capabilities(&self) -> LlmCaps {
        self.caps.clone()
    }

    fn complete(&self, _req: LlmRequest) -> AppResult<BoxStream<'_, AppResult<LlmChunk>>> {
        let last = self.chunks.len().saturating_sub(1);
        Ok(Box::new(self.chunks.iter().enumerate().map(
            move |(index, delta)| {
                Ok(LlmChunk {
                    delta: delta.clone(),
                    is_final: index == last,
                    usage: None,
                })
            },
        )))
    }
}
