mod support;

use std::sync::Arc;

use asr_local::{LocalAsrEngine, RecognitionTuning};
use dictation_api::AsrEngine;
use dictation_api::fakes::{FakeClock, FakeModelStore};
use dictation_types::{AppError, AppResult, ErrorKind, ModelDescriptor, ModelKind};
use support::{
    ENGINE, Fixture, ScriptedBackend, entry_of_kind, speech_entry, tuning, with_setting,
};

fn model(id: &str, engine: &str) -> ModelDescriptor {
    speech_entry(
        id,
        engine,
        &["id", "uk", "bn", "he"],
        &[("sampleRateHz", "16000"), ("languageMixing", "mixed")],
    )
}

fn build(
    backend: &ScriptedBackend,
    model: ModelDescriptor,
    tuning: RecognitionTuning,
) -> AppResult<LocalAsrEngine> {
    LocalAsrEngine::new(
        Arc::new(backend.clone()),
        Arc::new(FakeModelStore::new(vec![model.clone()])),
        Arc::new(FakeClock::new(0)),
        model,
        tuning,
    )
}

fn backend(engine: &str) -> ScriptedBackend {
    ScriptedBackend::new(engine, FakeClock::new(0))
}

#[test]
fn a_model_made_for_the_backends_engine_gets_an_engine() {
    build(&backend(ENGINE), model("model-a", ENGINE), tuning()).expect("an engine");
}

#[test]
fn a_model_made_for_another_engine_is_refused() {
    let backend = backend("engine-x");
    for other in ["engine-y", "engine-x2", "Engine-X", "engine"] {
        let result = build(&backend, model("model-a", other), tuning());
        assert!(result.is_err(), "a model for '{other}' was accepted");
    }
    assert_eq!(backend.load_count(), 0);
}

#[test]
fn a_model_the_backend_cannot_check_is_refused_with_the_backends_reason() {
    let backend = backend(ENGINE);
    backend.refuse_check(AppError::validation("the entry names no vocabulary"));
    let err = build(&backend, model("model-a", ENGINE), tuning())
        .err()
        .expect("refused");
    assert_eq!(err.kind, ErrorKind::Validation);
    assert!(err.message.contains("names no vocabulary"), "{err}");
    assert_eq!(backend.load_count(), 0);
}

#[test]
fn invalid_tuning_is_refused() {
    let bad = [
        RecognitionTuning {
            thread_share: 0.0,
            ..tuning()
        },
        RecognitionTuning {
            thread_share: 1.5,
            ..tuning()
        },
        RecognitionTuning {
            thread_share: f32::NAN,
            ..tuning()
        },
        RecognitionTuning {
            execution_provider: String::new(),
            ..tuning()
        },
        RecognitionTuning {
            load_timeout_ms: 0,
            ..tuning()
        },
        RecognitionTuning {
            worker_parent_check_ms: 0,
            ..tuning()
        },
    ];
    for tuning in bad {
        assert!(
            build(&backend(ENGINE), model("model-a", ENGINE), tuning.clone()).is_err(),
            "an engine was built with {tuning:?}"
        );
    }
}

#[test]
fn an_entry_that_does_not_say_what_the_model_can_do_is_refused() {
    let backend = backend(ENGINE);
    let without_rate = with_setting(model("model-a", ENGINE), "sampleRateHz", None);
    assert!(build(&backend, without_rate, tuning()).is_err());
    let without_mixing = with_setting(model("model-a", ENGINE), "languageMixing", None);
    assert!(build(&backend, without_mixing, tuning()).is_err());
    let not_speech = entry_of_kind(
        ModelKind::Llm,
        "model-a",
        ENGINE,
        &["id", "uk"],
        &[("sampleRateHz", "16000"), ("languageMixing", "mixed")],
    );
    assert!(build(&backend, not_speech, tuning()).is_err());
}

#[test]
fn an_engine_can_be_built_before_the_model_is_installed() {
    let fixture = Fixture::not_installed(model("model-a", ENGINE));
    assert!(!fixture.engine.is_loaded());
    assert_eq!(fixture.backend.load_count(), 0);
}

#[test]
fn building_an_engine_loads_nothing() {
    let fixture = Fixture::installed(model("model-a", ENGINE));
    assert!(!fixture.engine.is_loaded());
    assert_eq!(fixture.backend.load_count(), 0);
    assert_eq!(fixture.backend.models_alive(), 0);
    assert!(!fixture.store_says_loaded());
}

#[test]
fn the_provider_id_names_the_engine_and_the_model() {
    for (id, engine_name) in [
        ("model-a", "engine-x"),
        ("model-b", "engine-x"),
        ("model-a", "engine-y"),
    ] {
        let engine = build(&backend(engine_name), model(id, engine_name), tuning()).unwrap();
        let provider = engine.id();
        assert!(provider.as_str().contains(engine_name), "{provider}");
        assert!(provider.as_str().contains(id), "{provider}");
    }
}

#[test]
fn engines_for_different_models_have_different_provider_ids() {
    let backend = backend(ENGINE);
    let first = build(&backend, model("model-a", ENGINE), tuning()).unwrap();
    let second = build(&backend, model("model-b", ENGINE), tuning()).unwrap();
    assert_ne!(first.id(), second.id());
}
