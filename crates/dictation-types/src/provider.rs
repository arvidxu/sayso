use crate::{
    AppError, AppResult, AsrCaps, ExtensionId, LlmCaps, ModelId, ProviderId, ProviderRestriction,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderKind {
    Asr,
    Llm,
}

/// The variant also says what kind of provider it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderCaps {
    Asr(AsrCaps),
    Llm(LlmCaps),
}

/// What registration adds; the engine supplies the ID and capabilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderDetails {
    pub display_name: String,
    pub requires_credential: bool,
    pub default_endpoint: Option<String>,
    pub supported_models: Vec<ModelId>,
}

impl ProviderDetails {
    pub fn new(display_name: impl Into<String>, requires_credential: bool) -> Self {
        Self {
            display_name: display_name.into(),
            requires_credential,
            default_endpoint: None,
            supported_models: Vec::new(),
        }
    }

    pub fn describe(self, id: ProviderId, caps: ProviderCaps) -> ProviderDescriptor {
        ProviderDescriptor {
            id,
            display_name: self.display_name,
            caps,
            requires_credential: self.requires_credential,
            default_endpoint: self.default_endpoint,
            supported_models: self.supported_models,
            extension_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderDescriptor {
    pub id: ProviderId,
    pub display_name: String,
    pub caps: ProviderCaps,
    pub requires_credential: bool,
    pub default_endpoint: Option<String>,
    pub supported_models: Vec<ModelId>,
    /// `None` for built-in providers.
    pub extension_id: Option<ExtensionId>,
}

impl ProviderDescriptor {
    pub fn asr(
        id: ProviderId,
        display_name: impl Into<String>,
        caps: AsrCaps,
        requires_credential: bool,
    ) -> Self {
        Self::new(
            id,
            display_name,
            requires_credential,
            ProviderCaps::Asr(caps),
        )
    }

    pub fn llm(
        id: ProviderId,
        display_name: impl Into<String>,
        caps: LlmCaps,
        requires_credential: bool,
    ) -> Self {
        Self::new(
            id,
            display_name,
            requires_credential,
            ProviderCaps::Llm(caps),
        )
    }

    fn new(
        id: ProviderId,
        display_name: impl Into<String>,
        requires_credential: bool,
        caps: ProviderCaps,
    ) -> Self {
        Self {
            id,
            display_name: display_name.into(),
            caps,
            requires_credential,
            default_endpoint: None,
            supported_models: Vec::new(),
            extension_id: None,
        }
    }

    pub fn kind(&self) -> ProviderKind {
        match self.caps {
            ProviderCaps::Asr(_) => ProviderKind::Asr,
            ProviderCaps::Llm(_) => ProviderKind::Llm,
        }
    }

    pub fn asr_caps(&self) -> Option<&AsrCaps> {
        match &self.caps {
            ProviderCaps::Asr(caps) => Some(caps),
            ProviderCaps::Llm(_) => None,
        }
    }

    pub fn llm_caps(&self) -> Option<&LlmCaps> {
        match &self.caps {
            ProviderCaps::Llm(caps) => Some(caps),
            ProviderCaps::Asr(_) => None,
        }
    }

    pub fn with_default_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.default_endpoint = Some(endpoint.into());
        self
    }

    pub fn with_supported_models(mut self, models: Vec<ModelId>) -> Self {
        self.supported_models = models;
        self
    }

    pub fn with_requires_credential(mut self, requires_credential: bool) -> Self {
        self.requires_credential = requires_credential;
        self
    }

    pub fn validate(&self) -> AppResult<()> {
        if self.id.is_empty() {
            return Err(AppError::validation("provider ID must not be empty"));
        }
        if self.display_name.trim().is_empty() {
            return Err(AppError::validation(
                "provider display_name must not be empty",
            ));
        }
        if self
            .default_endpoint
            .as_deref()
            .is_some_and(|endpoint| endpoint.trim().is_empty())
        {
            return Err(AppError::validation(
                "provider default_endpoint must not be blank",
            ));
        }
        for (position, model) in self.supported_models.iter().enumerate() {
            if self.supported_models[..position].contains(model) {
                return Err(AppError::validation(format!(
                    "provider lists the model {model} twice"
                )));
            }
        }
        match &self.caps {
            ProviderCaps::Asr(caps) => caps.validate(),
            ProviderCaps::Llm(caps) => caps.validate(),
        }
    }

    /// Declared limits, for display.
    pub fn limits(&self) -> Vec<ProviderRestriction> {
        match &self.caps {
            ProviderCaps::Asr(caps) => caps.limits(),
            ProviderCaps::Llm(caps) if !caps.supports_streaming => {
                vec![ProviderRestriction::StreamingUnsupported]
            }
            ProviderCaps::Llm(_) => Vec::new(),
        }
    }
}
