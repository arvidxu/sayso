#![allow(dead_code)]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use dictation_api::{
    AsrEngine, AsrSession, AudioCaptureSession, AudioSource, BoxStream, LlmEngine, SecretStore,
};
use dictation_types::{
    AppError, AppResult, AsrCaps, AsrOptions, AudioCaptureConfig, AudioDevice, DeviceId, Languages,
    LlmCaps, LlmChunk, LlmMessage, LlmRequest, ProviderId, SecretString, Transcript, TranscriptId,
};

pub fn langs(spec: &str) -> Languages {
    Languages::parse(spec).unwrap_or_else(|e| panic!("'{spec}' should be a valid list: {e}"))
}

pub type Log = Arc<Mutex<Vec<String>>>;

#[derive(Clone, Copy, PartialEq)]
pub enum FailAt {
    Nowhere,
    Start,
    Push,
    Finish,
}

pub struct LoggingSession {
    pub log: Log,
    pub languages: Languages,
    pub fail_at: FailAt,
}

impl AsrSession for LoggingSession {
    fn push_audio(&mut self, samples: &[f32]) -> AppResult<Option<Transcript>> {
        self.log
            .lock()
            .unwrap()
            .push(format!("push:{}", samples.len()));
        if self.fail_at == FailAt::Push {
            return Err(AppError::asr("push failed"));
        }
        Ok(None)
    }

    fn finish(&mut self) -> AppResult<Transcript> {
        self.log.lock().unwrap().push("finish".to_owned());
        if self.fail_at == FailAt::Finish {
            return Err(AppError::timeout("finish failed"));
        }
        Ok(Transcript::new(
            TranscriptId::new("t-1"),
            "bom dia, buongiorno",
            self.languages.clone(),
        ))
    }
}

pub struct LoggingEngine {
    pub log: Log,
    pub fail_at: FailAt,
}

impl LoggingEngine {
    pub fn new(fail_at: FailAt) -> Self {
        Self {
            log: Log::default(),
            fail_at,
        }
    }
}

impl AsrEngine for LoggingEngine {
    fn id(&self) -> ProviderId {
        ProviderId::new("logging-engine")
    }

    fn capabilities(&self) -> AsrCaps {
        AsrCaps::new(langs("pt+it").iter().cloned().collect(), 16_000, true)
    }

    fn start(&self, opts: AsrOptions) -> AppResult<Box<dyn AsrSession>> {
        self.log
            .lock()
            .unwrap()
            .push(format!("start:{}", opts.languages));
        if self.fail_at == FailAt::Start {
            return Err(AppError::asr("start failed"));
        }
        Ok(Box::new(LoggingSession {
            log: Arc::clone(&self.log),
            languages: opts.languages,
            fail_at: self.fail_at,
        }))
    }
}

pub struct ChunkEngine {
    pub items: Vec<AppResult<LlmChunk>>,
    pub refuse: Option<AppError>,
}

impl ChunkEngine {
    pub fn new(items: Vec<AppResult<LlmChunk>>) -> Self {
        Self {
            items,
            refuse: None,
        }
    }
}

impl LlmEngine for ChunkEngine {
    fn id(&self) -> ProviderId {
        ProviderId::new("chunk-engine")
    }

    fn capabilities(&self) -> LlmCaps {
        LlmCaps::new(true, true)
    }

    fn complete(&self, _req: LlmRequest) -> AppResult<BoxStream<'_, AppResult<LlmChunk>>> {
        if let Some(err) = &self.refuse {
            return Err(err.clone());
        }
        Ok(Box::new(self.items.clone().into_iter()))
    }
}

pub fn request() -> LlmRequest {
    LlmRequest::new(vec![LlmMessage::user("réécris ceci")])
}

pub struct Devices(pub AppResult<Vec<AudioDevice>>);

impl AudioSource for Devices {
    fn list_devices(&self) -> AppResult<Vec<AudioDevice>> {
        self.0.clone()
    }

    fn start_capture(
        &self,
        _config: AudioCaptureConfig,
    ) -> AppResult<Box<dyn AudioCaptureSession>> {
        Err(AppError::unsupported("this double only lists devices"))
    }
}

pub fn device(id: &str, is_default: bool) -> AudioDevice {
    let mut device = AudioDevice::new(DeviceId::new(id), id, 48_000, 1);
    device.is_default = is_default;
    device
}

#[derive(Default)]
pub struct MapSecrets {
    pub secrets: Mutex<BTreeMap<String, SecretString>>,
    pub broken: bool,
}

impl SecretStore for MapSecrets {
    fn get_secret(&self, key: &str) -> AppResult<Option<SecretString>> {
        if self.broken {
            return Err(AppError::secret("secret store locked"));
        }
        Ok(self.secrets.lock().unwrap().get(key).cloned())
    }

    fn set_secret(&self, key: &str, secret: SecretString) -> AppResult<()> {
        self.secrets.lock().unwrap().insert(key.to_owned(), secret);
        Ok(())
    }

    fn delete_secret(&self, key: &str) -> AppResult<bool> {
        Ok(self.secrets.lock().unwrap().remove(key).is_some())
    }
}
