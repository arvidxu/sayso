use std::fmt;

use std::collections::BTreeMap;

use std::sync::Arc;

use dictation_api::{AsrEngine, LlmEngine};
use dictation_types::{
    AppError, AppResult, AsrCaps, AsrOptions, LlmCaps, ProviderDescriptor, ProviderId,
    ProviderKind, ProviderRestriction,
};

#[derive(Default, Clone)]
pub struct ProviderRegistry {
    descriptors: BTreeMap<ProviderId, ProviderDescriptor>,
    asr_engines: BTreeMap<ProviderId, Arc<dyn AsrEngine>>,
    llm_engines: BTreeMap<ProviderId, Arc<dyn LlmEngine>>,
}

impl fmt::Debug for ProviderRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProviderRegistry")
            .field("descriptors", &self.descriptors)
            .finish_non_exhaustive()
    }
}

impl ProviderRegistry {
    /// Checks `descriptor` and makes room for it, removing any engine registered under its ID.
    fn admit(&mut self, descriptor: ProviderDescriptor, replace: bool) -> AppResult<ProviderId> {
        descriptor.validate()?;
        if !replace && self.descriptors.contains_key(&descriptor.id) {
            return Err(AppError::already_exists(format!(
                "provider '{}' is already registered",
                descriptor.id
            )));
        }
        let id = descriptor.id.clone();
        self.asr_engines.remove(&id);
        self.llm_engines.remove(&id);
        self.descriptors.insert(id.clone(), descriptor);
        Ok(id)
    }

    pub(crate) fn insert_asr(
        &mut self,
        descriptor: ProviderDescriptor,
        engine: Arc<dyn AsrEngine>,
        replace: bool,
    ) -> AppResult<()> {
        let id = self.admit(descriptor, replace)?;
        self.asr_engines.insert(id, engine);
        Ok(())
    }

    pub(crate) fn insert_llm(
        &mut self,
        descriptor: ProviderDescriptor,
        engine: Arc<dyn LlmEngine>,
        replace: bool,
    ) -> AppResult<()> {
        let id = self.admit(descriptor, replace)?;
        self.llm_engines.insert(id, engine);
        Ok(())
    }

    pub(crate) fn remove(&mut self, id: &ProviderId) -> bool {
        self.asr_engines.remove(id);
        self.llm_engines.remove(id);
        self.descriptors.remove(id).is_some()
    }

    pub fn get_descriptor(&self, id: &ProviderId) -> Option<&ProviderDescriptor> {
        self.descriptors.get(id)
    }

    pub fn get_asr(&self, id: &ProviderId) -> Option<Arc<dyn AsrEngine>> {
        self.asr_engines.get(id).cloned()
    }

    pub fn get_llm(&self, id: &ProviderId) -> Option<Arc<dyn LlmEngine>> {
        self.llm_engines.get(id).cloned()
    }

    pub fn asr_capabilities(&self, id: &ProviderId) -> Option<&AsrCaps> {
        self.descriptors.get(id).and_then(|d| d.asr_caps())
    }

    pub fn llm_capabilities(&self, id: &ProviderId) -> Option<&LlmCaps> {
        self.descriptors.get(id).and_then(|d| d.llm_caps())
    }

    /// The limits provider `id` declares, for display; `None` when it is not registered.
    pub fn provider_limits(&self, id: &ProviderId) -> Option<Vec<ProviderRestriction>> {
        self.descriptors.get(id).map(ProviderDescriptor::limits)
    }

    /// The parts of `opts` (and of the recording length, when known) that speech provider `id`
    /// cannot handle; empty when the request is fine.
    pub fn check_asr_options(
        &self,
        id: &ProviderId,
        opts: &AsrOptions,
        duration_secs: Option<u32>,
    ) -> AppResult<Vec<ProviderRestriction>> {
        let caps = self.asr_capabilities(id).ok_or_else(|| {
            AppError::not_found(format!("speech provider '{id}' is not registered"))
        })?;
        Ok(caps.check_options(opts, duration_secs))
    }

    pub fn list_descriptors(&self) -> Vec<&ProviderDescriptor> {
        self.descriptors.values().collect()
    }

    pub fn list_by_kind(&self, kind: ProviderKind) -> Vec<&ProviderDescriptor> {
        self.descriptors
            .values()
            .filter(|d| d.kind() == kind)
            .collect()
    }
}
