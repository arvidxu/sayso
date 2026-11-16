//! Model capabilities, read only from catalog engine settings.

use std::str::FromStr;

use dictation_types::{AppError, AppResult, AsrCaps, LanguageMixing, ModelDescriptor, ModelKind};

/// In Hz.
pub const SAMPLE_RATE_HZ: &str = "sampleRateHz";

/// In seconds. Optional.
pub const MAX_AUDIO_SECS: &str = "maxAudioSecs";

/// `one` (must be told which), `auto-detect-one` (one per utterance, detected) or `mixed`
/// (languages may alternate within an utterance).
pub const LANGUAGE_MIXING: &str = "languageMixing";

/// How many languages a `mixed` model can mix. Optional.
pub const MAX_MIXED_LANGUAGES: &str = "maxMixedLanguages";

/// `true` or `false`; absent means false.
pub const SUPPORTS_HOTWORDS: &str = "supportsHotwords";

pub(crate) fn missing(model: &ModelDescriptor, name: &str) -> AppError {
    AppError::validation(format!("model {} has no engine setting '{name}'", model.id))
}

pub fn number<T: FromStr>(model: &ModelDescriptor, name: &str) -> AppResult<Option<T>> {
    model
        .engine_setting(name)
        .map(|text| {
            text.trim().parse::<T>().map_err(|_| {
                AppError::validation(format!(
                    "model {}: the engine setting '{name}' is not a usable number: '{text}'",
                    model.id
                ))
            })
        })
        .transpose()
}

pub fn capabilities_of(model: &ModelDescriptor) -> AppResult<AsrCaps> {
    if model.kind != ModelKind::Asr {
        return Err(AppError::validation(format!(
            "model {} is not a speech recognition model",
            model.id
        )));
    }
    let sample_rate_hz: u32 =
        number(model, SAMPLE_RATE_HZ)?.ok_or_else(|| missing(model, SAMPLE_RATE_HZ))?;
    let mut caps = AsrCaps::new(model.supported_languages.clone(), sample_rate_hz, true);
    caps.max_duration_secs = number(model, MAX_AUDIO_SECS)?;
    let mixing = model
        .engine_setting(LANGUAGE_MIXING)
        .ok_or_else(|| missing(model, LANGUAGE_MIXING))?;
    caps.language_mixing = match mixing.trim() {
        "one" => LanguageMixing::OnePerUtterance,
        "auto-detect-one" => LanguageMixing::AutoDetectOne,
        "mixed" => LanguageMixing::Mixed {
            max_languages: number(model, MAX_MIXED_LANGUAGES)?,
        },
        other => {
            return Err(AppError::validation(format!(
                "model {}: unknown {LANGUAGE_MIXING} '{other}'",
                model.id
            )));
        }
    };
    caps.supports_hotwords = match model.engine_setting(SUPPORTS_HOTWORDS).map(str::trim) {
        None | Some("false") => false,
        Some("true") => true,
        Some(other) => {
            return Err(AppError::validation(format!(
                "model {}: {SUPPORTS_HOTWORDS} must be true or false, not '{other}'",
                model.id
            )));
        }
    };
    caps.validate()?;
    Ok(caps)
}
