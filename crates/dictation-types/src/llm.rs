use crate::{AppError, AppResult, ModelId, TokenUsage};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmCaps {
    pub supports_streaming: bool,
    pub max_context_tokens: Option<u32>,
    pub operates_locally: bool,
}

impl LlmCaps {
    /// An engine with no declared context limit.
    pub fn new(operates_locally: bool, supports_streaming: bool) -> Self {
        Self {
            supports_streaming,
            max_context_tokens: None,
            operates_locally,
        }
    }

    pub fn validate(&self) -> AppResult<()> {
        if self.max_context_tokens == Some(0) {
            return Err(AppError::validation(
                "LLM max_context_tokens must be greater than 0 when specified",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LlmRole {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmMessage {
    pub role: LlmRole,
    pub content: String,
}

impl LlmMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: LlmRole::System,
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: LlmRole::User,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: LlmRole::Assistant,
            content: content.into(),
        }
    }
}

/// Settings left `None` are up to the engine.
#[derive(Debug, Clone, PartialEq)]
pub struct LlmRequest {
    pub model_id: Option<ModelId>,
    pub messages: Vec<LlmMessage>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stream: Option<bool>,
}

impl LlmRequest {
    pub fn new(messages: Vec<LlmMessage>) -> Self {
        Self {
            model_id: None,
            messages,
            temperature: None,
            max_tokens: None,
            stream: None,
        }
    }

    pub fn with_model(mut self, model_id: ModelId) -> Self {
        self.model_id = Some(model_id);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmChunk {
    pub delta: String,
    pub is_final: bool,
    /// Usually only on the last chunk.
    pub usage: Option<TokenUsage>,
}

impl LlmChunk {
    pub fn text(delta: impl Into<String>) -> Self {
        Self {
            delta: delta.into(),
            is_final: false,
            usage: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmCompletion {
    pub text: String,
    pub usage: Option<TokenUsage>,
}
