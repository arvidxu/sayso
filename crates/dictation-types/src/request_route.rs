use crate::{ModelId, ProviderId};

/// Lets the user see what left the device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestRoute {
    pub stage: String,
    pub provider_id: ProviderId,
    pub model_id: Option<ModelId>,
    /// `None` when the request stayed on the device.
    pub endpoint: Option<String>,
}

impl RequestRoute {
    pub fn local(
        stage: impl Into<String>,
        provider_id: ProviderId,
        model_id: Option<ModelId>,
    ) -> Self {
        Self {
            stage: stage.into(),
            provider_id,
            model_id,
            endpoint: None,
        }
    }

    pub fn remote(
        stage: impl Into<String>,
        provider_id: ProviderId,
        model_id: Option<ModelId>,
        endpoint: impl Into<String>,
    ) -> Self {
        Self {
            stage: stage.into(),
            provider_id,
            model_id,
            endpoint: Some(endpoint.into()),
        }
    }

    pub fn is_local(&self) -> bool {
        self.endpoint.is_none()
    }
}
