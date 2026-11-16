// Each example uses only part of this.
#![allow(dead_code)]

mod clock;

pub use clock::SystemClock;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use asr_local::sherpa_onnx::SherpaOnnxBackend;
use asr_local::{LocalAsrEngine, ModelBackend, RecognitionTuning};
use audio::{
    AudioConfig, NeuralSpeechScorer, SegmentationConfig, VoiceDetector, resample_to_mono,
    segment_speech,
};
use dictation_api::{AsrEngine, ModelStore};
use dictation_types::{
    AppError, AppResult, AsrCaps, AsrOptions, AudioChunk, ModelDescriptor, ModelId,
    join_segment_texts,
};
use engine_worker::{WorkerBackend, WorkerCommand};
use model_store::{LocalModelStore, ModelCatalog, ModelsConfig};

const MS_PER_SECOND: u64 = 1_000;

const PCM: u16 = 1;
const FLOAT: u16 = 3;

/// Supports 16-bit integer and 32-bit float samples only.
pub fn read_wav(path: &Path) -> AppResult<AudioChunk> {
    let bytes =
        fs::read(path).map_err(|err| AppError::storage(format!("{}: {err}", path.display())))?;
    let bad = || {
        AppError::validation(format!(
            "{} is not a WAV file with 16-bit or 32-bit float samples",
            path.display()
        ))
    };
    let u16_at = |at: usize| {
        bytes
            .get(at..at + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
    };
    let u32_at = |at: usize| {
        bytes
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    if bytes.get(..4) != Some(b"RIFF".as_slice()) || bytes.get(8..12) != Some(b"WAVE".as_slice()) {
        return Err(bad());
    }
    let (mut at, mut format, mut data) = (12usize, None, None);
    while let (Some(tag), Some(len)) = (bytes.get(at..at + 4), u32_at(at + 4)) {
        let body = at + 8;
        let end = body.saturating_add(len as usize).min(bytes.len());
        if tag == b"fmt " {
            if len < 16 {
                return Err(bad());
            }
            format = Some((
                u16_at(body).ok_or_else(bad)?,
                u16_at(body + 2).ok_or_else(bad)?,
                u32_at(body + 4).ok_or_else(bad)?,
                u16_at(body + 14).ok_or_else(bad)?,
            ));
        } else if tag == b"data" {
            data = Some(&bytes[body..end]);
        }
        at = end + (len as usize & 1);
    }
    let (Some((tag, channels, rate, bits)), Some(data)) = (format, data) else {
        return Err(bad());
    };
    if channels == 0 || rate == 0 {
        return Err(bad());
    }
    let samples: Vec<f32> = match (tag, bits) {
        (PCM, 16) => data
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| f32::from(i16::from_le_bytes(*pair)) / 32_768.0)
            .collect(),
        (FLOAT, 32) => data
            .as_chunks::<4>()
            .0
            .iter()
            .map(|four| f32::from_le_bytes(*four))
            .collect(),
        _ => return Err(bad()),
    };
    Ok(AudioChunk::new(samples, rate, channels))
}

/// A `--name` without a value is an error, so it is never left behind to be misread.
pub fn take_option(args: &mut Vec<String>, name: &str) -> AppResult<Option<String>> {
    let Some(at) = args.iter().position(|arg| arg == name) else {
        return Ok(None);
    };
    if at + 1 >= args.len() {
        return Err(AppError::validation(format!(
            "{name} needs a value after it"
        )));
    }
    let value = args.remove(at + 1);
    args.remove(at);
    Ok(Some(value))
}

/// The only accepted value is `at-pauses`.
pub fn take_cut(args: &mut Vec<String>) -> AppResult<bool> {
    match take_option(args, "--cut")?.as_deref() {
        None => Ok(false),
        Some("at-pauses") => Ok(true),
        Some(other) => Err(AppError::validation(format!(
            "--cut takes 'at-pauses', not '{other}'"
        ))),
    }
}

pub struct Recognised {
    pub text: String,
    pub audio_secs: f32,
    pub pieces: usize,
}

/// Set up the way the application does, with the shipped catalog and tuning files.
pub struct Recogniser {
    pub engine: LocalAsrEngine,
    pub model: ModelDescriptor,
    pub caps: AsrCaps,
    pub threads: usize,
    /// Recognise each stretch of speech between pauses separately, as live dictation does.
    /// Otherwise only audio longer than the model takes at once is cut.
    pub at_pauses: bool,
    pub show_pieces: bool,
    audio: AudioConfig,
    detector: VoiceDetector<NeuralSpeechScorer>,
}

impl Recogniser {
    /// Without `worker` the engine runs in-process; without `catalog` the shipped one is used.
    pub fn open(
        worker: Option<WorkerCommand>,
        catalog: Option<&Path>,
        id: &ModelId,
        folder: PathBuf,
    ) -> AppResult<Self> {
        let shipped = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../model-catalog.json");
        let audio = AudioConfig::bundled()?;
        let recognition = RecognitionTuning::bundled()?;
        let catalog = ModelCatalog::load(catalog.unwrap_or(&shipped))?;
        let model = catalog
            .get(id)
            .cloned()
            .ok_or_else(|| AppError::not_found(format!("model {id} is not in the catalog")))?;
        let store: Arc<dyn ModelStore> = Arc::new(LocalModelStore::new(
            catalog,
            folder,
            &ModelsConfig::bundled()?.download,
            Vec::new(),
        )?);
        let in_process: Arc<dyn ModelBackend> = Arc::new(SherpaOnnxBackend::new());
        let backend: Arc<dyn ModelBackend> = match worker {
            Some(command) => Arc::new(WorkerBackend::new(in_process, command)),
            None => in_process,
        };
        let engine = LocalAsrEngine::new(
            backend,
            store,
            Arc::new(SystemClock::new()),
            model.clone(),
            recognition.clone(),
        )?;
        let detector = VoiceDetector::new(
            audio.voice_detection.clone(),
            NeuralSpeechScorer::new(),
            audio.resampling.clone(),
        )?;
        Ok(Self {
            caps: engine.capabilities(),
            engine,
            model,
            threads: recognition.threads(),
            at_pauses: false,
            show_pieces: false,
            audio,
            detector,
        })
    }

    /// Cut the way the app cuts, when the model has a length limit.
    fn limits(&self) -> Option<SegmentationConfig> {
        let max_secs = self.caps.max_duration_secs?;
        Some(
            self.audio
                .segmentation
                .for_max(u64::from(max_secs) * MS_PER_SECOND),
        )
    }

    /// Lengths are compared in samples so no recording slips past the limit by rounding.
    fn pieces(&mut self, sound: &AudioChunk) -> AppResult<Vec<AudioChunk>> {
        let limits = self.limits();
        if self.at_pauses {
            let mut pieces = Vec::new();
            for span in self.detector.detect(sound)? {
                match &limits {
                    Some(limits) => pieces.extend(
                        segment_speech(sound, std::slice::from_ref(&span), limits)?
                            .into_iter()
                            .map(|piece| piece.audio),
                    ),
                    None => pieces.push(AudioChunk::new(
                        sound.samples[span.start_frame..span.end_frame].to_vec(),
                        sound.sample_rate_hz,
                        sound.channels,
                    )),
                }
            }
            return Ok(pieces);
        }
        let most = self
            .caps
            .max_duration_secs
            .map(|secs| u64::from(secs).saturating_mul(u64::from(sound.sample_rate_hz)) as usize);
        match (limits, most) {
            (Some(limits), Some(most)) if sound.samples.len() > most => {
                let spans = self.detector.detect(sound)?;
                Ok(segment_speech(sound, &spans, &limits)?
                    .into_iter()
                    .map(|piece| piece.audio)
                    .collect())
            }
            _ => Ok(vec![sound.clone()]),
        }
    }

    /// A file longer than the model takes at once is cut at pauses and the texts joined.
    pub fn recognise(&mut self, file: &Path, options: &AsrOptions) -> AppResult<Recognised> {
        let recording = read_wav(file)?;
        let sound = resample_to_mono(&recording, self.caps.sample_rate_hz, &self.audio.resampling)?;
        let pieces = self.pieces(&sound)?;
        let mut segments = Vec::new();
        for piece in &pieces {
            let transcript = self.engine.transcribe(&piece.samples, options.clone())?;
            if self.show_pieces {
                let secs = piece.duration_ms() as f32 / MS_PER_SECOND as f32;
                println!("    [{secs:.1} s] {}", transcript.raw_text);
            }
            segments.extend(transcript.segments);
        }
        Ok(Recognised {
            text: join_segment_texts(&segments),
            audio_secs: sound.duration_ms() as f32 / MS_PER_SECOND as f32,
            pieces: pieces.len(),
        })
    }
}
