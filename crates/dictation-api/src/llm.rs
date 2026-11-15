use dictation_types::{AppResult, LlmCaps, LlmChunk, LlmCompletion, LlmRequest, ProviderId};

/// An iterator, so streaming needs no async runtime.
pub type BoxStream<'a, T> = Box<dyn Iterator<Item = T> + Send + 'a>;

pub trait LlmEngine: Send + Sync {
    fn id(&self) -> ProviderId;

    fn capabilities(&self) -> LlmCaps;

    fn complete(&self, req: LlmRequest) -> AppResult<BoxStream<'_, AppResult<LlmChunk>>>;

    /// Runs a request and returns the whole answer, with the token usage the engine reported
    /// last. A failure anywhere in the stream fails the call.
    fn complete_text(&self, req: LlmRequest) -> AppResult<LlmCompletion> {
        let mut completion = LlmCompletion {
            text: String::new(),
            usage: None,
        };
        for chunk in self.complete(req)? {
            let chunk = chunk?;
            completion.text.push_str(&chunk.delta);
            if chunk.usage.is_some() {
                completion.usage = chunk.usage;
            }
        }
        Ok(completion)
    }
}
