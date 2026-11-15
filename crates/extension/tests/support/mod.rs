#![allow(dead_code)]

use std::sync::Arc;

use dictation_api::fakes::{FakeAsrEngine, FakeLlmEngine};
use dictation_api::{AsrEngine, LlmEngine};
use dictation_types::{
    AppResult, AsrCaps, AsrOptions, ExtensionId, FeatureDescriptor, FeatureId, FeatureMountPoint,
    LanguageMixing, Languages, LlmCaps, LlmMessage, LlmRequest, Mode, ModeId, ProviderDescriptor,
    ProviderDetails, ProviderId, Transcript, TranscriptId,
};
use extension::{Extension, Registry};

pub fn langs(spec: &str) -> Languages {
    Languages::parse(spec).unwrap_or_else(|e| panic!("'{spec}' should be a valid list: {e}"))
}

pub fn pid(id: &str) -> ProviderId {
    ProviderId::new(id)
}

/// Capabilities of an engine that mixes the languages of `spec` freely.
pub fn speech_caps(spec: &str) -> AsrCaps {
    let mut caps = AsrCaps::new(langs(spec).as_slice().to_vec(), 16_000, true);
    caps.language_mixing = LanguageMixing::Mixed {
        max_languages: None,
    };
    caps
}

/// A speech engine for the languages of `spec` that always recognises `says`.
pub fn speech_engine(id: &str, spec: &str, says: &str) -> Arc<FakeAsrEngine> {
    let transcript = Transcript::new(TranscriptId::new("t-1"), says, langs(spec));
    Arc::new(FakeAsrEngine::new(pid(id), speech_caps(spec), transcript))
}

pub fn speech_details(engine: &FakeAsrEngine) -> ProviderDetails {
    ProviderDetails::new(format!("Speech {}", engine.id()), false)
}

/// A language model engine that always answers `answer`.
pub fn writer_engine(id: &str, answer: &str) -> Arc<FakeLlmEngine> {
    Arc::new(FakeLlmEngine::new(
        pid(id),
        LlmCaps::new(true, true),
        vec![answer.to_owned()],
    ))
}

pub fn writer_details(engine: &FakeLlmEngine) -> ProviderDetails {
    ProviderDetails::new(format!("Writer {}", engine.id()), false)
}

pub fn register_speech(registry: &mut Registry, id: &str, spec: &str, says: &str) -> AppResult<()> {
    let engine = speech_engine(id, spec, says);
    registry.register_asr_provider(speech_details(&engine), engine)
}

pub fn upsert_speech(registry: &mut Registry, id: &str, spec: &str, says: &str) -> AppResult<()> {
    let engine = speech_engine(id, spec, says);
    registry.upsert_asr_provider(speech_details(&engine), engine)
}

pub fn register_writer(registry: &mut Registry, id: &str, answer: &str) -> AppResult<()> {
    let engine = writer_engine(id, answer);
    registry.register_llm_provider(writer_details(&engine), engine)
}

pub fn upsert_writer(registry: &mut Registry, id: &str, answer: &str) -> AppResult<()> {
    let engine = writer_engine(id, answer);
    registry.upsert_llm_provider(writer_details(&engine), engine)
}

/// What the speech engine registered as `id` recognises, found by running it.
pub fn said_by(registry: &Registry, id: &str) -> Option<String> {
    let engine = registry.providers().get_asr(&pid(id))?;
    let languages = Languages::new(engine.capabilities().supported_languages).unwrap();
    Some(
        engine
            .transcribe(&[0.0; 16], AsrOptions::new(languages))
            .unwrap()
            .raw_text,
    )
}

/// What the language model engine registered as `id` answers, found by running it.
pub fn written_by(registry: &Registry, id: &str) -> Option<String> {
    let engine = registry.providers().get_llm(&pid(id))?;
    let request = LlmRequest::new(vec![LlmMessage::user("anything")]);
    Some(engine.complete_text(request).unwrap().text)
}

pub fn provider_entry<'a>(registry: &'a Registry, id: &str) -> Option<&'a ProviderDescriptor> {
    registry.providers().get_descriptor(&pid(id))
}

pub fn mode_entry<'a>(registry: &'a Registry, id: &str) -> Option<&'a Mode> {
    registry.modes().get(&ModeId::new(id))
}

pub fn feature_entry<'a>(registry: &'a Registry, id: &str) -> Option<&'a FeatureDescriptor> {
    registry.features().get(&FeatureId::new(id))
}

fn as_text(source: Option<&ExtensionId>) -> Option<String> {
    source.map(|e| e.as_str().to_owned())
}

pub fn provider_source(registry: &Registry, id: &str) -> Option<String> {
    let entry = provider_entry(registry, id);
    let entry = entry.unwrap_or_else(|| panic!("provider '{id}' should be registered"));
    as_text(entry.extension_id.as_ref())
}

pub fn feature_source(registry: &Registry, id: &str) -> Option<String> {
    let entry = feature_entry(registry, id);
    let entry = entry.unwrap_or_else(|| panic!("feature '{id}' should be registered"));
    as_text(entry.extension_id.as_ref())
}

pub fn mode_source(registry: &Registry, id: &str) -> Option<String> {
    assert!(
        mode_entry(registry, id).is_some(),
        "mode '{id}' should be registered"
    );
    as_text(registry.mode_source(&ModeId::new(id)))
}

pub fn plain_mode(id: &str) -> Mode {
    Mode::without_llm(ModeId::new(id), format!("Mode {id}"))
}

pub fn llm_mode(id: &str, prompt: &str) -> Mode {
    Mode::with_llm(ModeId::new(id), format!("Mode {id}"), prompt)
}

pub fn feature(id: &str, mount_point: FeatureMountPoint, order: i32) -> FeatureDescriptor {
    FeatureDescriptor::new(
        FeatureId::new(id),
        format!("feature.{id}.title"),
        mount_point,
        true,
    )
    .with_order(order)
}

pub fn panel(id: &str) -> FeatureDescriptor {
    feature(id, FeatureMountPoint::FloatingPanel, 0)
}

/// An extension whose `register` is a closure.
pub struct FnExtension<F> {
    id: String,
    body: F,
}

impl<F> Extension for FnExtension<F>
where
    F: Fn(&mut Registry) -> AppResult<()> + Send + Sync,
{
    fn id(&self) -> &str {
        &self.id
    }

    fn register(&self, registry: &mut Registry) -> AppResult<()> {
        (self.body)(registry)
    }
}

pub fn extension<F>(id: &str, body: F) -> FnExtension<F>
where
    F: Fn(&mut Registry) -> AppResult<()> + Send + Sync,
{
    FnExtension {
        id: id.to_owned(),
        body,
    }
}

/// A registry with built-in entries of every kind, registered outside any extension.
pub fn registry_with_built_ins() -> Registry {
    let mut registry = Registry::new();
    register_speech(
        &mut registry,
        "built-in-speech",
        "pt+it",
        "bom dia, buongiorno",
    )
    .unwrap();
    register_writer(&mut registry, "built-in-writer", "built-in answer").unwrap();
    registry.register_mode(plain_mode("built-in-mode")).unwrap();
    registry
        .register_feature(panel("built-in-feature"))
        .unwrap();
    registry
}

/// An add-on that uses only the public interface, as an outside crate would.
pub struct TranslationAddOn;

impl Extension for TranslationAddOn {
    fn id(&self) -> &str {
        "translation-add-on"
    }

    fn register(&self, registry: &mut Registry) -> AppResult<()> {
        register_speech(
            registry,
            "add-on-speech",
            "ru+ko+fr",
            "привет 안녕하세요 bonjour",
        )?;
        register_writer(registry, "add-on-writer", "translated text")?;
        registry.register_mode(llm_mode(
            "add-on-translate",
            "Translate {text} into {target_language}",
        ))?;
        registry.register_feature(feature(
            "add-on-glossary",
            FeatureMountPoint::SettingsSection,
            40,
        ))?;
        Ok(())
    }
}

/// Adds, replaces and removes entries of every kind in a [`populated_registry`].
pub fn meddle(registry: &mut Registry) -> AppResult<()> {
    register_speech(registry, "meddler-speech", "bn+en", "আমি office যাচ্ছি")?;
    register_writer(registry, "meddler-writer", "meddled")?;
    registry.register_mode(llm_mode("meddler-mode", "Rewrite {text}"))?;
    registry.register_feature(panel("meddler-feature"))?;
    upsert_speech(registry, "built-in-speech", "ko", "교체됨")?;
    upsert_speech(registry, "built-in-writer", "ko", "now a speech engine")?;
    registry.upsert_mode(llm_mode("built-in-mode", "Replaced {text}"))?;
    upsert_writer(registry, "earlier-writer", "replaced earlier answer")?;
    registry.upsert_mode(llm_mode("earlier-mode", "Replaced {text}"))?;
    assert!(registry.unregister_provider(&pid("earlier-speech")));
    assert!(
        registry
            .unregister_mode(&ModeId::new("earlier-second-mode"))
            .is_some()
    );
    assert!(
        registry
            .unregister_feature(&FeatureId::new("built-in-feature"))
            .is_some()
    );
    assert!(
        registry
            .unregister_feature(&FeatureId::new("earlier-feature"))
            .is_some()
    );
    Ok(())
}

/// Built-ins plus an extension that loaded successfully before the one under test.
pub fn populated_registry() -> Registry {
    let mut registry = registry_with_built_ins();
    let earlier = extension("earlier", |registry| {
        register_speech(
            registry,
            "earlier-speech",
            "ru+ko+fr",
            "привет 안녕하세요 bonjour",
        )?;
        register_writer(registry, "earlier-writer", "earlier answer")?;
        registry.register_mode(llm_mode("earlier-mode", "Earlier {text}"))?;
        registry.register_mode(plain_mode("earlier-second-mode"))?;
        registry.register_feature(panel("earlier-feature"))
    });
    registry.load_extension(&earlier).unwrap();
    registry
}

/// Everything observable about what is registered, to compare before and after a load.
pub fn observe(registry: &Registry) -> Vec<String> {
    let mut seen = vec![
        format!("{:?}", registry.features().list()),
        format!("{:?}", registry.loaded_extensions()),
    ];
    for d in registry.providers().list_descriptors() {
        let (said, written) = (
            said_by(registry, d.id.as_str()),
            written_by(registry, d.id.as_str()),
        );
        seen.push(format!("{d:?} says {said:?} writes {written:?}"));
    }
    for mode in registry.modes().list() {
        seen.push(format!(
            "{mode:?} from {:?}",
            registry.mode_source(&mode.id)
        ));
    }
    seen
}

pub fn ids(descriptors: &[&ProviderDescriptor]) -> Vec<String> {
    descriptors
        .iter()
        .map(|d| d.id.as_str().to_owned())
        .collect()
}
