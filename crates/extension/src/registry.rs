use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use crate::{Extension, FeatureRegistry, ModeRegistry, ProviderRegistry};
use dictation_api::{AsrEngine, LlmEngine};
use dictation_types::{
    AppError, AppResult, ExtensionId, FeatureDescriptor, FeatureId, Mode, ModeId, ProviderCaps,
    ProviderDetails, ProviderId,
};

/// All changes go through this type, so that entries made while an extension loads are
/// attributed to it and can be undone together.
#[derive(Debug, Default)]
pub struct Registry {
    providers: ProviderRegistry,
    modes: ModeRegistry,
    features: FeatureRegistry,
    mode_sources: BTreeMap<ModeId, ExtensionId>,
    loaded_extensions: Vec<ExtensionId>,
    loading: Option<ExtensionId>,
    /// Identifies this very registry while an extension is loading into it. It cannot be
    /// read or copied from outside, so a registry assembled elsewhere never carries it.
    load_token: Option<Arc<()>>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn providers(&self) -> &ProviderRegistry {
        &self.providers
    }

    pub fn modes(&self) -> &ModeRegistry {
        &self.modes
    }

    pub fn features(&self) -> &FeatureRegistry {
        &self.features
    }

    pub fn loaded_extensions(&self) -> &[ExtensionId] {
        &self.loaded_extensions
    }

    /// IDs are compared without regard to ASCII letter case, so two spellings of one ID cannot
    /// both be loaded.
    pub fn has_extension(&self, id: &str) -> bool {
        self.loaded_extensions
            .iter()
            .any(|e| e.as_str().eq_ignore_ascii_case(id.trim()))
    }

    /// The extension that registered or last replaced mode `id`; `None` for built-in modes.
    pub fn mode_source(&self, id: &ModeId) -> Option<&ExtensionId> {
        self.mode_sources.get(id)
    }

    /// The engine supplies the ID and capabilities; the provider is attributed to the extension
    /// loading now.
    fn add_asr(
        &mut self,
        details: ProviderDetails,
        engine: Arc<dyn AsrEngine>,
        replace: bool,
    ) -> AppResult<()> {
        let mut descriptor =
            details.describe(engine.id(), ProviderCaps::Asr(engine.capabilities()));
        descriptor.extension_id = self.loading.clone();
        self.providers.insert_asr(descriptor, engine, replace)
    }

    /// Like [`Self::add_asr`], for a language model engine.
    fn add_llm(
        &mut self,
        details: ProviderDetails,
        engine: Arc<dyn LlmEngine>,
        replace: bool,
    ) -> AppResult<()> {
        let mut descriptor =
            details.describe(engine.id(), ProviderCaps::Llm(engine.capabilities()));
        descriptor.extension_id = self.loading.clone();
        self.providers.insert_llm(descriptor, engine, replace)
    }

    /// Fails when the ID the engine reports is taken.
    pub fn register_asr_provider(
        &mut self,
        details: ProviderDetails,
        engine: Arc<dyn AsrEngine>,
    ) -> AppResult<()> {
        self.add_asr(details, engine, false)
    }

    /// Fails when the ID the engine reports is taken.
    pub fn register_llm_provider(
        &mut self,
        details: ProviderDetails,
        engine: Arc<dyn LlmEngine>,
    ) -> AppResult<()> {
        self.add_llm(details, engine, false)
    }

    pub fn upsert_asr_provider(
        &mut self,
        details: ProviderDetails,
        engine: Arc<dyn AsrEngine>,
    ) -> AppResult<()> {
        self.add_asr(details, engine, true)
    }

    pub fn upsert_llm_provider(
        &mut self,
        details: ProviderDetails,
        engine: Arc<dyn LlmEngine>,
    ) -> AppResult<()> {
        self.add_llm(details, engine, true)
    }

    pub fn unregister_provider(&mut self, id: &ProviderId) -> bool {
        self.providers.remove(id)
    }

    fn insert_mode(&mut self, mode: Mode, replace: bool) -> AppResult<()> {
        let id = mode.id.clone();
        self.modes.insert(mode, replace)?;
        match &self.loading {
            Some(extension) => self.mode_sources.insert(id, extension.clone()),
            None => self.mode_sources.remove(&id),
        };
        Ok(())
    }

    /// Fails when the mode is invalid or its ID is taken.
    pub fn register_mode(&mut self, mode: Mode) -> AppResult<()> {
        self.insert_mode(mode, false)
    }

    pub fn upsert_mode(&mut self, mode: Mode) -> AppResult<()> {
        self.insert_mode(mode, true)
    }

    pub fn unregister_mode(&mut self, id: &ModeId) -> Option<Mode> {
        self.mode_sources.remove(id);
        self.modes.remove(id)
    }

    /// Fails when it has no ID or title, or its ID is taken.
    pub fn register_feature(&mut self, mut feature: FeatureDescriptor) -> AppResult<()> {
        feature.extension_id = self.loading.clone();
        self.features.insert(feature)
    }

    pub fn unregister_feature(&mut self, id: &FeatureId) -> Option<FeatureDescriptor> {
        self.features.remove(id)
    }

    /// Loads an extension and attributes everything it registers to it. If it returns an error,
    /// panics, or replaces the registry it was handed, the registry is restored unchanged.
    pub fn load_extension(&mut self, ext: &dyn Extension) -> AppResult<()> {
        let reported_id = catch_unwind(AssertUnwindSafe(|| ext.id().to_owned()))
            .map_err(|_| AppError::internal("an extension panicked while reporting its ID"))?;
        let ext_id = ExtensionId::try_new(reported_id)?;
        if self.has_extension(ext_id.as_str()) {
            return Err(AppError::already_exists(format!(
                "extension '{ext_id}' is already loaded"
            )));
        }
        if self.loading.is_some() {
            return Err(AppError::unsupported(
                "an extension cannot load another extension while it is being loaded",
            ));
        }

        let before = Self {
            providers: self.providers.clone(),
            modes: self.modes.clone(),
            features: self.features.clone(),
            mode_sources: self.mode_sources.clone(),
            loaded_extensions: self.loaded_extensions.clone(),
            loading: None,
            load_token: None,
        };
        let token = Arc::new(());
        self.load_token = Some(Arc::clone(&token));
        self.loading = Some(ext_id.clone());
        let outcome = catch_unwind(AssertUnwindSafe(|| ext.register(self)));

        // Only this registry holds the token, so a registry swapped in during the load lacks it.
        let is_same_registry = self
            .load_token
            .as_ref()
            .is_some_and(|held| Arc::ptr_eq(held, &token));
        let result = match outcome {
            Err(_) => Err(AppError::internal(format!(
                "extension '{ext_id}' panicked while registering"
            ))),
            Ok(_) if !is_same_registry => Err(AppError::validation(format!(
                "extension '{ext_id}' replaced the registry instead of registering into it"
            ))),
            Ok(result) => result,
        };
        if let Err(err) = result {
            *self = before;
            return Err(err);
        }
        self.load_token = None;
        self.loading = None;
        self.loaded_extensions.push(ext_id);
        Ok(())
    }
}
