#![cfg(feature = "sherpa-onnx")]
//! Entries start from a complete shipped-catalog entry so required names are not repeated.

use std::path::Path;

use asr_local::model_facts::{LANGUAGE_MIXING, MAX_AUDIO_SECS};
use asr_local::sherpa_onnx::SherpaOnnxBackend;
use asr_local::{ModelBackend, capabilities_of};
use dictation_types::{ErrorKind, ModelDescriptor, ModelId};
use model_store::ModelCatalog;

const TOTAL: &str = "maxTotalLen";
const NEW_TOKENS: &str = "maxNewTokens";
const TOKENS_PER_SEC: &str = "audioTokensPerSec";
const PROMPT: &str = "promptTokens";

const FAMILY: &str = "family";

fn backend() -> SherpaOnnxBackend {
    SherpaOnnxBackend::new()
}

fn complete_entry() -> ModelDescriptor {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../model-catalog.json");
    let catalog = ModelCatalog::load(&path).expect("the shipped catalog is valid");
    let mut entry = catalog
        .entries()
        .iter()
        .find(|entry| entry.engine == backend().engine_id())
        .expect("the shipped catalog has a model for this engine")
        .clone();
    entry.id = ModelId::new("model-a");
    entry.name = "Test model".to_owned();
    entry
}

fn with_setting(mut entry: ModelDescriptor, name: &str, value: Option<&str>) -> ModelDescriptor {
    entry.engine_settings.retain(|(n, _)| n != name);
    if let Some(value) = value {
        entry
            .engine_settings
            .push((name.to_owned(), value.to_owned()));
    }
    entry
}

fn whole_number(entry: &ModelDescriptor, name: &str) -> u64 {
    entry
        .engine_setting(name)
        .unwrap_or_else(|| panic!("the complete entry gives {name}"))
        .parse()
        .unwrap_or_else(|_| panic!("{name} is a whole number"))
}

fn with_limits(audio_secs: u64, new_tokens: u64, total: u64) -> ModelDescriptor {
    let entry = with_setting(
        complete_entry(),
        MAX_AUDIO_SECS,
        Some(&audio_secs.to_string()),
    );
    let entry = with_setting(entry, NEW_TOKENS, Some(&new_tokens.to_string()));
    with_setting(entry, TOTAL, Some(&total.to_string()))
}

fn assert_refused(what: &str, entry: &ModelDescriptor) {
    assert!(
        backend().check(entry).is_err(),
        "an entry with {what} was accepted"
    );
}

#[test]
fn the_check_does_not_depend_on_the_models_id_or_name() {
    for id in ["model-a", "model-b", "another.model_2"] {
        let mut entry = complete_entry();
        entry.id = ModelId::new(id);
        entry.name = format!("Test model {id}");
        backend().check(&entry).expect("accepted");
    }
}

#[test]
fn an_unknown_family_is_refused_by_name() {
    for family in ["some-other-family", "", "  "] {
        let entry = with_setting(complete_entry(), FAMILY, Some(family));
        let err = backend().check(&entry).expect_err("unknown family");
        assert_eq!(err.kind, ErrorKind::Unsupported, "{family:?}: {err}");
        assert!(err.message.contains(&format!("'{family}'")), "{err}");
    }
}

#[test]
fn a_missing_family_is_refused_as_an_incomplete_entry() {
    let err = backend()
        .check(&with_setting(complete_entry(), FAMILY, None))
        .expect_err("refused");
    assert_eq!(err.kind, ErrorKind::Validation, "{err}");
}

#[test]
fn every_missing_named_path_is_refused() {
    let complete = complete_entry();
    assert!(!complete.named_paths.is_empty());
    for (name, _) in &complete.named_paths {
        let mut entry = complete.clone();
        entry.named_paths.retain(|(n, _)| n != name);
        let err = backend()
            .check(&entry)
            .err()
            .unwrap_or_else(|| panic!("an entry without the path '{name}' was accepted"));
        assert_eq!(err.kind, ErrorKind::Validation, "{name}: {err}");
        assert!(err.message.contains(name.as_str()), "{name}: {err}");
    }
}

#[test]
fn every_missing_engine_setting_is_refused() {
    let complete = complete_entry();
    for (name, _) in &complete.engine_settings {
        let entry = with_setting(complete.clone(), name, None);
        if name == LANGUAGE_MIXING {
            // Checked as a capability, not by the backend.
            assert!(capabilities_of(&entry).is_err());
            continue;
        }
        if name.starts_with("languageName.") || name == "supportsHotwords" {
            backend()
                .check(&entry)
                .expect("an optional setting may be left out");
            continue;
        }
        let err = backend()
            .check(&entry)
            .err()
            .unwrap_or_else(|| panic!("an entry without the setting '{name}' was accepted"));
        assert_eq!(err.kind, ErrorKind::Validation, "{name}: {err}");
        assert!(err.message.contains(name.as_str()), "{name}: {err}");
    }
}

/// Includes the evaluation-only catalog, so every family is covered.
fn shipped_entries() -> Vec<ModelDescriptor> {
    ["model-catalog.json", "evaluation-models.json"]
        .iter()
        .flat_map(|file| {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(file);
            ModelCatalog::load(&path)
                .unwrap_or_else(|err| panic!("{file} is not a valid catalog: {err}"))
                .entries()
                .to_vec()
        })
        .filter(|entry| entry.engine == backend().engine_id())
        .collect()
}

#[test]
fn models_of_every_shipped_family_need_all_the_settings_their_engine_uses() {
    let entries = shipped_entries();
    let families: Vec<&str> = entries
        .iter()
        .filter_map(|entry| entry.engine_setting("family"))
        .collect();
    assert!(
        families.len() > 1,
        "only one family is shipped: {families:?}"
    );
    for complete in &entries {
        backend()
            .check(complete)
            .expect("a shipped entry is complete");
        for (name, _) in &complete.engine_settings {
            // Describe the model to the rest of the app or are optional; loading does not need them.
            let not_for_loading = name == LANGUAGE_MIXING
                || name == "supportsHotwords"
                || name == "maxAudioSecs"
                || name == "audioTokensPerSec"
                || name == "promptTokens"
                || name.starts_with("languageName.");
            let entry = with_setting(complete.clone(), name, None);
            if not_for_loading {
                continue;
            }
            let err = backend().check(&entry).err().unwrap_or_else(|| {
                panic!("{}: accepted without the setting '{name}'", complete.id)
            });
            assert_eq!(
                err.kind,
                ErrorKind::Validation,
                "{}: {name}: {err}",
                complete.id
            );
            assert!(
                err.message.contains(name.as_str()),
                "{}: {name}: {err}",
                complete.id
            );
        }
    }
}

#[test]
fn every_number_setting_that_is_not_a_number_is_refused() {
    let complete = complete_entry();
    for (name, value) in &complete.engine_settings {
        if value.parse::<f64>().is_err() {
            continue;
        }
        assert_refused(
            &format!("'{name}' given as words"),
            &with_setting(complete.clone(), name, Some("plenty")),
        );
    }
}

#[test]
fn limits_that_fit_exactly_are_accepted() {
    let complete = complete_entry();
    let per_sec = whole_number(&complete, TOKENS_PER_SEC);
    let prompt = whole_number(&complete, PROMPT);
    for (audio_secs, new_tokens) in [(1, 1), (10, 100), (30, 200), (7, 311)] {
        let total = audio_secs * per_sec + prompt + new_tokens;
        backend()
            .check(&with_limits(audio_secs, new_tokens, total))
            .unwrap_or_else(|err| {
                panic!("{audio_secs} s and {new_tokens} tokens in {total} were refused: {err}")
            });
        backend()
            .check(&with_limits(audio_secs, new_tokens, total + 1))
            .expect("room to spare is fine");
    }
}

#[test]
fn limits_that_do_not_fit_the_total_are_refused() {
    let complete = complete_entry();
    let per_sec = whole_number(&complete, TOKENS_PER_SEC);
    let prompt = whole_number(&complete, PROMPT);
    for (audio_secs, new_tokens) in [(1, 1), (10, 100), (30, 200), (7, 311)] {
        let total = audio_secs * per_sec + prompt + new_tokens;
        assert_refused(
            "a total one token too small",
            &with_limits(audio_secs, new_tokens, total - 1),
        );
        assert_refused(
            "audio one second too long",
            &with_limits(audio_secs + 1, new_tokens, total),
        );
        assert_refused(
            "text one token too long",
            &with_limits(audio_secs, new_tokens + 1, total),
        );
    }
}

#[test]
fn a_prompt_or_audio_that_takes_more_room_than_the_total_is_refused() {
    let complete = complete_entry();
    let total = whole_number(&complete, TOTAL);
    assert_refused(
        "a prompt as large as the total",
        &with_setting(complete.clone(), PROMPT, Some(&total.to_string())),
    );
    assert_refused(
        "a second of audio as large as the total",
        &with_setting(complete, TOKENS_PER_SEC, Some(&total.to_string())),
    );
}

#[test]
fn a_model_that_may_write_no_text_is_refused() {
    let complete = complete_entry();
    assert_refused(
        "no room for text",
        &with_setting(complete, NEW_TOKENS, Some("0")),
    );
}
