//! One JSON line per message; a recognise request is followed by raw sample bytes. Replies
//! carry [`REPLY_MARK`] so engine text on the same stream can be told apart.

use std::io::{BufRead, Read, Write};
use std::str::FromStr;

use asr_local::RecognitionTuning;
use dictation_types::{
    AppError, AppResult, ArchiveFormat, AsrOptions, ErrorKind, Language, Languages,
    ModelDescriptor, ModelFile, ModelId, ModelKind, PlatformTarget, ProviderId, Term, TermId,
    TermKind,
};
use serde::{Deserialize, Serialize};

pub(crate) const REPLY_MARK: &str = "engine-worker-reply ";

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum Request {
    Load(Box<LoadRequest>),
    /// Followed by `samples` samples, each a little-endian `f32`.
    Recognise(RecogniseRequest),
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct LoadRequest {
    pub model: WireModel,
    pub dir: String,
    pub thread_share: f32,
    pub execution_provider: String,
    pub load_timeout_ms: u64,
    pub worker_parent_check_ms: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RecogniseRequest {
    pub sample_rate_hz: u32,
    pub samples: usize,
    /// In the format [`Languages`] writes.
    pub languages: String,
    /// Only their spelling crosses to the worker.
    pub hotwords: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum Reply {
    Loaded,
    Text(String),
    Failed(WireError),
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct WireError {
    pub kind: String,
    pub message: String,
    pub retryable: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct WireModel {
    id: String,
    name: String,
    kind: String,
    engine: String,
    group: Option<String>,
    files: Vec<WireFile>,
    provided_by_system: bool,
    named_paths: Vec<(String, String)>,
    engine_settings: Vec<(String, String)>,
    supported_platforms: Vec<String>,
    supported_languages: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireFile {
    path: String,
    size_bytes: u64,
    sha256: String,
    download_urls: Vec<String>,
    unpack: Option<String>,
}

const KIND_ASR: &str = "asr";
const KIND_LLM: &str = "llm";
const KIND_VAD: &str = "vad";

impl From<&ModelDescriptor> for WireModel {
    fn from(model: &ModelDescriptor) -> Self {
        let kind = match model.kind {
            ModelKind::Asr => KIND_ASR,
            ModelKind::Llm => KIND_LLM,
            ModelKind::Vad => KIND_VAD,
        };
        Self {
            id: model.id.as_str().to_owned(),
            name: model.name.clone(),
            kind: kind.to_owned(),
            engine: model.engine.as_str().to_owned(),
            group: model.group.clone(),
            files: model
                .files
                .iter()
                .map(|file| WireFile {
                    path: file.path.clone(),
                    size_bytes: file.size_bytes,
                    sha256: file.sha256.clone(),
                    download_urls: file.download_urls.clone(),
                    unpack: file.unpack.map(|format| format.as_str().to_owned()),
                })
                .collect(),
            provided_by_system: model.provided_by_system,
            named_paths: model.named_paths.clone(),
            engine_settings: model.engine_settings.clone(),
            supported_platforms: model
                .supported_platforms
                .iter()
                .map(|platform| platform.as_str().to_owned())
                .collect(),
            supported_languages: model
                .supported_languages
                .iter()
                .map(Language::code)
                .collect(),
        }
    }
}

impl WireModel {
    /// The main program checked the entry before sending it.
    pub(crate) fn into_descriptor(self) -> AppResult<ModelDescriptor> {
        let kind = match self.kind.as_str() {
            KIND_ASR => ModelKind::Asr,
            KIND_LLM => ModelKind::Llm,
            KIND_VAD => ModelKind::Vad,
            other => {
                return Err(AppError::validation(format!(
                    "unknown kind of model '{other}'"
                )));
            }
        };
        let files = self
            .files
            .into_iter()
            .map(|file| {
                Ok(ModelFile {
                    path: file.path,
                    size_bytes: file.size_bytes,
                    sha256: file.sha256,
                    download_urls: file.download_urls,
                    unpack: file
                        .unpack
                        .as_deref()
                        .map(ArchiveFormat::from_str)
                        .transpose()?,
                })
            })
            .collect::<AppResult<Vec<_>>>()?;
        Ok(ModelDescriptor {
            id: ModelId::new(self.id),
            name: self.name,
            kind,
            engine: ProviderId::new(self.engine),
            group: self.group,
            files,
            provided_by_system: self.provided_by_system,
            named_paths: self.named_paths,
            engine_settings: self.engine_settings,
            supported_platforms: self
                .supported_platforms
                .iter()
                .map(|platform| PlatformTarget::from_str(platform))
                .collect::<AppResult<Vec<_>>>()?,
            supported_languages: self
                .supported_languages
                .iter()
                .map(|language| Language::parse(language))
                .collect::<AppResult<Vec<_>>>()?,
        })
    }
}

impl LoadRequest {
    pub(crate) fn tuning(&self) -> RecognitionTuning {
        RecognitionTuning {
            thread_share: self.thread_share,
            execution_provider: self.execution_provider.clone(),
            load_timeout_ms: self.load_timeout_ms,
            worker_parent_check_ms: self.worker_parent_check_ms,
        }
    }
}

impl RecogniseRequest {
    /// The engine has already checked the rest of `opts`; models only use these parts.
    pub(crate) fn new(samples: usize, sample_rate_hz: u32, opts: &AsrOptions) -> Self {
        Self {
            sample_rate_hz,
            samples,
            languages: opts.languages.code(),
            hotwords: opts
                .hotwords
                .iter()
                .filter(|term| term.enabled)
                .map(|term| term.canonical.clone())
                .collect(),
        }
    }

    pub(crate) fn options(&self) -> AppResult<AsrOptions> {
        let mut opts = AsrOptions::new(Languages::parse(&self.languages)?);
        let language = opts.languages.primary().clone();
        for (number, word) in self.hotwords.iter().enumerate() {
            opts.hotwords.push(Term::new(
                TermId::new(number.to_string()),
                word,
                TermKind::default(),
                language.clone(),
            )?);
        }
        Ok(opts)
    }
}

/// A kind without a name here crosses as its own text.
const KINDS: &[(&str, ErrorKind)] = &[
    ("validation", ErrorKind::Validation),
    ("not-found", ErrorKind::NotFound),
    ("already-exists", ErrorKind::AlreadyExists),
    ("permission-denied", ErrorKind::PermissionDenied(None)),
    ("audio-device", ErrorKind::AudioDevice),
    ("asr-engine", ErrorKind::AsrEngine),
    ("llm-engine", ErrorKind::LlmEngine),
    ("text-injection", ErrorKind::TextInjection),
    ("hotkey", ErrorKind::Hotkey),
    ("storage", ErrorKind::Storage),
    ("secret-store", ErrorKind::SecretStore),
    ("model-store", ErrorKind::ModelStore),
    ("checksum-mismatch", ErrorKind::ChecksumMismatch),
    ("network", ErrorKind::Network),
    ("rate-limited", ErrorKind::RateLimited),
    ("timeout", ErrorKind::Timeout),
    ("cancelled", ErrorKind::Cancelled),
    ("unsupported", ErrorKind::Unsupported),
    ("internal", ErrorKind::Internal),
];

impl From<&AppError> for WireError {
    fn from(err: &AppError) -> Self {
        let kind = match &err.kind {
            ErrorKind::PermissionDenied(_) => ErrorKind::PermissionDenied(None),
            other => other.clone(),
        };
        let name = KINDS
            .iter()
            .find(|(_, known)| *known == kind)
            .map_or_else(|| kind.to_string(), |(name, _)| (*name).to_owned());
        Self {
            kind: name,
            message: err.message.clone(),
            retryable: err.retryable,
        }
    }
}

impl From<WireError> for AppError {
    fn from(err: WireError) -> Self {
        let kind = KINDS
            .iter()
            .find(|(name, _)| *name == err.kind)
            .map_or_else(
                || ErrorKind::Custom(err.kind.clone()),
                |(_, kind)| kind.clone(),
            );
        AppError::new(kind, err.message).with_retryable(err.retryable)
    }
}

fn broken(what: &str, err: impl ToString) -> AppError {
    AppError::internal(format!("{what}: {}", err.to_string()))
}

pub(crate) fn write_line<T: Serialize>(
    out: &mut impl Write,
    mark: &str,
    message: &T,
) -> AppResult<()> {
    let text = serde_json::to_string(message).map_err(|err| broken("message not written", err))?;
    out.write_all(mark.as_bytes())
        .and_then(|()| out.write_all(text.as_bytes()))
        .and_then(|()| out.write_all(b"\n"))
        .and_then(|()| out.flush())
        .map_err(|err| broken("message not sent", err))
}

/// `None` when the other side is gone. Engine text on the same stream may be in any encoding
/// (often the system code page), so a line is decoded only once it is known to hold a reply.
pub(crate) fn read_raw_line(input: &mut impl BufRead) -> AppResult<Option<Vec<u8>>> {
    let mut line = Vec::new();
    let read = input
        .read_until(b'\n', &mut line)
        .map_err(|err| broken("message not received", err))?;
    if read == 0 {
        return Ok(None);
    }
    while line
        .last()
        .is_some_and(|byte| matches!(byte, b'\r' | b'\n'))
    {
        line.pop();
    }
    Ok(Some(line))
}

/// Bytes that are not UTF-8 are rejected.
pub(crate) fn parse_bytes<'a, T: Deserialize<'a>>(line: &'a [u8]) -> AppResult<T> {
    serde_json::from_slice(line).map_err(|err| broken("message not understood", err))
}

pub(crate) fn write_samples(out: &mut impl Write, samples: &[f32]) -> AppResult<()> {
    let mut bytes = Vec::with_capacity(samples.len().saturating_mul(size_of::<f32>()));
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    out.write_all(&bytes)
        .and_then(|()| out.flush())
        .map_err(|err| broken("audio not sent", err))
}

/// Memory grows only as samples arrive, so a wrong count fails instead of over-allocating.
pub(crate) fn read_samples(input: &mut impl Read, count: usize) -> AppResult<Vec<f32>> {
    let wanted = count
        .checked_mul(size_of::<f32>())
        .ok_or_else(|| broken("audio not received", "too many samples"))?;
    let mut bytes = Vec::new();
    input
        .take(wanted as u64)
        .read_to_end(&mut bytes)
        .map_err(|err| broken("audio not received", err))?;
    if bytes.len() != wanted {
        return Err(broken("audio not received", "the audio ended early"));
    }
    let (samples, _) = bytes.as_chunks::<{ size_of::<f32>() }>();
    Ok(samples
        .iter()
        .map(|bytes| f32::from_le_bytes(*bytes))
        .collect())
}
