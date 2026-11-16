use std::path::{Component, Path};
use std::str::FromStr;
use std::sync::{Mutex, PoisonError};

use dictation_types::{AppError, AppResult, AsrOptions, CancelToken, ModelDescriptor, ProviderId};
use sherpa_onnx::{
    OfflineFunASRNanoModelConfig, OfflineModelConfig, OfflineQwen3ASRModelConfig,
    OfflineRecognizer, OfflineRecognizerConfig, OfflineSenseVoiceModelConfig,
    OfflineTransducerModelConfig, OfflineWhisperModelConfig,
};

use crate::language_names::LanguageNames;
use crate::model_facts::{MAX_AUDIO_SECS, SAMPLE_RATE_HZ, missing, number};
use crate::{ModelBackend, RecognitionTuning, SpeechModel};

const ENGINE_ID: &str = "sherpa-onnx";

const FAMILY: &str = "family";

const FAMILY_QWEN3_ASR: &str = "qwen3-asr";
const FAMILY_SENSE_VOICE: &str = "sense-voice";
const FAMILY_WHISPER: &str = "whisper";
const FAMILY_NEMO_TRANSDUCER: &str = "nemo-transducer";
const FAMILY_FUNASR_NANO: &str = "funasr-nano";

/// Whether numbers, dates and the like are normalised to written form: `true` or `false`.
const USE_ITN: &str = "useItn";
/// Silence appended after the audio, in frames.
const TAIL_PADDINGS: &str = "tailPaddings";
const DECODING_METHOD: &str = "decodingMethod";
const SYSTEM_PROMPT: &str = "systemPrompt";
const USER_PROMPT: &str = "userPrompt";

const WHISPER_TASK: &str = "transcribe";
const NEMO_TRANSDUCER_TYPE: &str = "nemo_transducer";

/// In tokens: prompt, audio and output together. These settings are all required; the
/// engine's built-in defaults are never used.
const MAX_TOTAL_LEN: &str = "maxTotalLen";
/// In tokens.
const MAX_NEW_TOKENS: &str = "maxNewTokens";
const AUDIO_TOKENS_PER_SEC: &str = "audioTokensPerSec";
/// Tokens taken by the fixed part of the prompt.
const PROMPT_TOKENS: &str = "promptTokens";
const TEMPERATURE: &str = "temperature";
const TOP_P: &str = "topP";
const SEED: &str = "seed";
/// Must match the feature size the model was trained on.
const FEATURE_DIM: &str = "featureDim";

const OPTION_LANGUAGE: &str = "language";

const OPTION_HOTWORDS: &str = "hotwords";

const HOTWORD_SEPARATOR: &str = ",";

#[derive(Debug, Clone, Copy, Default)]
pub struct SherpaOnnxBackend;

impl SherpaOnnxBackend {
    pub fn new() -> Self {
        Self
    }
}

fn setting<'a>(model: &'a ModelDescriptor, name: &str) -> AppResult<&'a str> {
    model
        .engine_setting(name)
        .ok_or_else(|| missing(model, name))
}

/// A path that would lead out of `dir` is refused.
fn named(model: &ModelDescriptor, dir: &Path, name: &str) -> AppResult<String> {
    let relative = model.named_path(name).ok_or_else(|| {
        AppError::validation(format!("model {} names no path '{name}'", model.id))
    })?;
    let inside = Path::new(relative)
        .components()
        .all(|part| matches!(part, Component::Normal(_) | Component::CurDir));
    if !inside {
        return Err(AppError::validation(format!(
            "model {} names a path '{name}' outside its folder",
            model.id
        )));
    }
    dir.join(relative)
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| AppError::validation("the model's folder has a name the engine cannot read"))
}

const PATH_MODEL: &str = "model";
const PATH_TOKENS: &str = "tokens";
const PATH_JOINER: &str = "joiner";
const PATH_ENCODER_ADAPTOR: &str = "encoder_adaptor";
const PATH_LLM: &str = "llm";
const PATH_EMBEDDING: &str = "embedding";
const PATH_CONV_FRONTEND: &str = "conv_frontend";
const PATH_ENCODER: &str = "encoder";
const PATH_DECODER: &str = "decoder";
const PATH_TOKENIZER: &str = "tokenizer";

fn required<T: FromStr>(model: &ModelDescriptor, name: &str) -> AppResult<T> {
    number(model, name)?.ok_or_else(|| missing(model, name))
}

/// Also returns the tokens left free once the longest audio, prompt and output are counted.
fn qwen3_asr(model: &ModelDescriptor, dir: &Path) -> AppResult<(OfflineQwen3ASRModelConfig, u64)> {
    let config = OfflineQwen3ASRModelConfig {
        conv_frontend: Some(named(model, dir, PATH_CONV_FRONTEND)?),
        encoder: Some(named(model, dir, PATH_ENCODER)?),
        decoder: Some(named(model, dir, PATH_DECODER)?),
        tokenizer: Some(named(model, dir, PATH_TOKENIZER)?),
        max_total_len: required(model, MAX_TOTAL_LEN)?,
        max_new_tokens: required(model, MAX_NEW_TOKENS)?,
        temperature: required(model, TEMPERATURE)?,
        top_p: required(model, TOP_P)?,
        seed: required(model, SEED)?,
        hotwords: None,
    };
    // If audio, prompt and output do not fit together the engine silently truncates the audio.
    let max_audio_secs: u64 = required(model, MAX_AUDIO_SECS)?;
    let per_sec: u64 = required(model, AUDIO_TOKENS_PER_SEC)?;
    let prompt: u64 = required(model, PROMPT_TOKENS)?;
    let needed = max_audio_secs
        .saturating_mul(per_sec)
        .saturating_add(prompt)
        .saturating_add(u64::try_from(config.max_new_tokens).unwrap_or(0));
    if config.max_new_tokens <= 0 || needed > u64::try_from(config.max_total_len).unwrap_or(0) {
        return Err(AppError::validation(format!(
            "model {}: {MAX_AUDIO_SECS} of audio, the prompt and {MAX_NEW_TOKENS} need {needed} \
             tokens, more than {MAX_TOTAL_LEN} allows",
            model.id
        )));
    }
    let spare = u64::try_from(config.max_total_len)
        .unwrap_or(0)
        .saturating_sub(needed);
    Ok((config, spare))
}

fn flag(model: &ModelDescriptor, name: &str) -> AppResult<bool> {
    match setting(model, name)?.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(AppError::validation(format!(
            "model {}: the engine setting '{name}' must be true or false, not '{other}'",
            model.id
        ))),
    }
}

/// The model detects the language itself.
fn sense_voice(
    model: &ModelDescriptor,
    dir: &Path,
    config: &mut OfflineModelConfig,
) -> AppResult<()> {
    config.sense_voice = OfflineSenseVoiceModelConfig {
        model: Some(named(model, dir, PATH_MODEL)?),
        language: None,
        use_itn: flag(model, USE_ITN)?,
    };
    config.tokens = Some(named(model, dir, PATH_TOKENS)?);
    Ok(())
}

/// The model detects the language itself.
fn whisper(model: &ModelDescriptor, dir: &Path, config: &mut OfflineModelConfig) -> AppResult<()> {
    config.whisper = OfflineWhisperModelConfig {
        encoder: Some(named(model, dir, PATH_ENCODER)?),
        decoder: Some(named(model, dir, PATH_DECODER)?),
        language: None,
        task: Some(WHISPER_TASK.to_owned()),
        tail_paddings: required(model, TAIL_PADDINGS)?,
        ..OfflineWhisperModelConfig::default()
    };
    config.tokens = Some(named(model, dir, PATH_TOKENS)?);
    Ok(())
}

fn nemo_transducer(
    model: &ModelDescriptor,
    dir: &Path,
    config: &mut OfflineModelConfig,
) -> AppResult<()> {
    config.transducer = OfflineTransducerModelConfig {
        encoder: Some(named(model, dir, PATH_ENCODER)?),
        decoder: Some(named(model, dir, PATH_DECODER)?),
        joiner: Some(named(model, dir, PATH_JOINER)?),
    };
    config.tokens = Some(named(model, dir, PATH_TOKENS)?);
    config.model_type = Some(NEMO_TRANSDUCER_TYPE.to_owned());
    Ok(())
}

fn funasr_nano(model: &ModelDescriptor, dir: &Path) -> AppResult<OfflineFunASRNanoModelConfig> {
    Ok(OfflineFunASRNanoModelConfig {
        encoder_adaptor: Some(named(model, dir, PATH_ENCODER_ADAPTOR)?),
        llm: Some(named(model, dir, PATH_LLM)?),
        embedding: Some(named(model, dir, PATH_EMBEDDING)?),
        tokenizer: Some(named(model, dir, PATH_TOKENIZER)?),
        system_prompt: Some(setting(model, SYSTEM_PROMPT)?.to_owned()),
        user_prompt: Some(setting(model, USER_PROMPT)?.to_owned()),
        max_new_tokens: required(model, MAX_NEW_TOKENS)?,
        temperature: required(model, TEMPERATURE)?,
        top_p: required(model, TOP_P)?,
        seed: required(model, SEED)?,
        language: None,
        itn: i32::from(flag(model, USE_ITN)?),
        hotwords: None,
    })
}

#[derive(Default)]
struct Limits {
    max_new_tokens: Option<usize>,
    /// Tokens free for hotwords.
    spare_tokens: usize,
}

fn recognizer_config(
    model: &ModelDescriptor,
    dir: &Path,
) -> AppResult<(OfflineRecognizerConfig, Limits)> {
    let mut config = OfflineRecognizerConfig::default();
    let limits = match setting(model, FAMILY)? {
        FAMILY_QWEN3_ASR => {
            let (family_config, spare) = qwen3_asr(model, dir)?;
            let limits = Limits {
                max_new_tokens: Some(usize::try_from(family_config.max_new_tokens).unwrap_or(0)),
                spare_tokens: usize::try_from(spare).unwrap_or(usize::MAX),
            };
            config.model_config.qwen3_asr = family_config;
            config.feat_config.feature_dim = required(model, FEATURE_DIM)?;
            limits
        }
        FAMILY_SENSE_VOICE => {
            sense_voice(model, dir, &mut config.model_config)?;
            Limits::default()
        }
        FAMILY_WHISPER => {
            whisper(model, dir, &mut config.model_config)?;
            Limits::default()
        }
        FAMILY_FUNASR_NANO => {
            let family_config = funasr_nano(model, dir)?;
            let limits = Limits {
                max_new_tokens: Some(usize::try_from(family_config.max_new_tokens).unwrap_or(0)),
                spare_tokens: 0,
            };
            config.model_config.funasr_nano = family_config;
            limits
        }
        FAMILY_NEMO_TRANSDUCER => {
            nemo_transducer(model, dir, &mut config.model_config)?;
            config.decoding_method = Some(setting(model, DECODING_METHOD)?.to_owned());
            Limits::default()
        }
        other => {
            return Err(AppError::unsupported(format!(
                "model {}: this build cannot run models of the family '{other}'",
                model.id
            )));
        }
    };
    config.feat_config.sample_rate = required(model, SAMPLE_RATE_HZ)?;
    Ok((config, limits))
}

impl ModelBackend for SherpaOnnxBackend {
    fn engine_id(&self) -> ProviderId {
        ProviderId::new(ENGINE_ID)
    }

    fn check(&self, model: &ModelDescriptor) -> AppResult<()> {
        recognizer_config(model, Path::new(""))?;
        LanguageNames::of(model).map(|_| ())
    }

    fn load(
        &self,
        model: &ModelDescriptor,
        dir: &Path,
        tuning: &RecognitionTuning,
        cancel: &CancelToken,
    ) -> AppResult<Box<dyn SpeechModel>> {
        // The engine cannot stop mid-load, so only an earlier cancel is honoured.
        if cancel.is_cancelled() {
            return Err(AppError::cancelled("loading the model was stopped"));
        }
        let (mut config, limits) = recognizer_config(model, dir)?;
        let language_names = LanguageNames::of(model)?;
        for (name, relative) in &model.named_paths {
            if !dir.join(relative).exists() {
                return Err(AppError::model(format!(
                    "model {} is missing '{name}' at '{relative}'",
                    model.id
                )));
            }
        }
        config.model_config.num_threads = i32::try_from(tuning.threads()).unwrap_or(1);
        config.model_config.provider = Some(tuning.execution_provider.clone());
        let recognizer = OfflineRecognizer::create(&config).ok_or_else(|| {
            AppError::asr(format!(
                "the engine could not load model {} from '{}'",
                model.id,
                dir.display()
            ))
        })?;
        Ok(Box::new(LoadedRecognizer {
            recognizer: Mutex::new(recognizer),
            limits,
            language_names,
        }))
    }
}

struct LoadedRecognizer {
    /// One recognition at a time: running several at once on one model is no faster.
    recognizer: Mutex<OfflineRecognizer>,
    limits: Limits,
    language_names: LanguageNames,
}

impl LoadedRecognizer {
    /// Hotwords go into the prompt, so only those fitting the spare tokens are kept; byte
    /// length is used as an upper bound on token count.
    fn hotwords(&self, opts: &AsrOptions) -> Option<String> {
        let mut text = String::new();
        for term in opts.hotwords.iter().filter(|term| term.enabled) {
            let word = term.canonical.trim();
            if word.is_empty() || word.contains(HOTWORD_SEPARATOR) || word.contains('\0') {
                continue;
            }
            let separator = if text.is_empty() {
                ""
            } else {
                HOTWORD_SEPARATOR
            };
            let longer = text.len() + separator.len() + word.len();
            if longer > self.limits.spare_tokens {
                break;
            }
            text.push_str(separator);
            text.push_str(word);
        }
        (!text.is_empty()).then_some(text)
    }
}

impl SpeechModel for LoadedRecognizer {
    fn recognise(
        &self,
        samples: &[f32],
        sample_rate_hz: u32,
        opts: &AsrOptions,
        cancel: &CancelToken,
    ) -> AppResult<String> {
        // The engine cannot stop mid-recognition, so only an earlier cancel is honoured.
        if cancel.is_cancelled() {
            return Err(AppError::cancelled("the recognition was stopped"));
        }
        let rate = i32::try_from(sample_rate_hz)
            .map_err(|_| AppError::validation("the sample rate is too high for the engine"))?;
        let recognizer = self
            .recognizer
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let stream = recognizer.create_stream();
        if let Some(hotwords) = self.hotwords(opts) {
            stream.set_option(OPTION_HOTWORDS, &hotwords);
        }
        if let Some(language) = self.language_names.hint_for(opts) {
            stream.set_option(OPTION_LANGUAGE, language);
        }
        stream.accept_waveform(rate, samples);
        recognizer.decode(&stream);
        let result = stream
            .get_result()
            .ok_or_else(|| AppError::asr("the engine returned no result"))?;
        if self
            .limits
            .max_new_tokens
            .is_some_and(|most| result.tokens.len() >= most)
        {
            // Output hit the token limit, so its end is missing.
            return Err(AppError::asr(
                "the recording holds more speech than the model can write out at once",
            ));
        }
        Ok(result.text)
    }
}
