#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use audio::{FrameScorer, SpeechSpan, VoiceDetectionConfig, VoiceDetector};
use dictation_types::{AppResult, AudioChunk};

mod pieces;

#[allow(unused_imports)]
pub use pieces::{assert_pieces_faithful, spans_ms};

/// (sample rate, frame length); frames of 20 ms, 16 ms and 10 ms.
pub const SCORER_FORMATS: [(u32, usize); 3] = [(8_000, 160), (16_000, 256), (48_000, 480)];

/// (sample rate, channels).
pub const INPUT_FORMATS: [(u32, u16); 10] = [
    (8_000, 1),
    (8_000, 2),
    (16_000, 1),
    (16_000, 2),
    (22_050, 1),
    (22_050, 2),
    (44_100, 1),
    (44_100, 2),
    (48_000, 1),
    (48_000, 2),
];

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Reset,
    Frame(Vec<f32>),
}

pub type Log = Arc<Mutex<Vec<Event>>>;

type Rule = Box<dyn FnMut(usize, &[f32]) -> AppResult<f32> + Send>;

/// The rule gets the frame index since the last reset and the frame.
pub struct ScriptedScorer {
    sample_rate_hz: u32,
    frame_len: usize,
    next_index: usize,
    rule: Rule,
    log: Log,
}

impl ScriptedScorer {
    pub fn new(
        sample_rate_hz: u32,
        frame_len: usize,
        rule: impl FnMut(usize, &[f32]) -> AppResult<f32> + Send + 'static,
    ) -> (Self, Log) {
        let log = Log::default();
        let scorer = Self {
            sample_rate_hz,
            frame_len,
            next_index: 0,
            rule: Box::new(rule),
            log: Arc::clone(&log),
        };
        (scorer, log)
    }

    /// Answers 0 past the end of the list.
    pub fn from_scores(sample_rate_hz: u32, frame_len: usize, scores: Vec<f32>) -> (Self, Log) {
        Self::new(sample_rate_hz, frame_len, move |index, _| {
            Ok(scores.get(index).copied().unwrap_or(0.0))
        })
    }

    /// `floor` is a root mean square.
    pub fn by_loudness(sample_rate_hz: u32, frame_len: usize, floor: f32) -> (Self, Log) {
        Self::new(sample_rate_hz, frame_len, move |_, frame| {
            Ok(if rms(frame) > floor { 1.0 } else { 0.0 })
        })
    }

    pub fn constant(sample_rate_hz: u32, frame_len: usize, score: f32) -> (Self, Log) {
        Self::new(sample_rate_hz, frame_len, move |_, _| Ok(score))
    }
}

impl FrameScorer for ScriptedScorer {
    fn sample_rate_hz(&self) -> u32 {
        self.sample_rate_hz
    }

    fn frame_len(&self) -> usize {
        self.frame_len
    }

    fn score(&mut self, frame: &[f32]) -> AppResult<f32> {
        self.log.lock().unwrap().push(Event::Frame(frame.to_vec()));
        let index = self.next_index;
        self.next_index += 1;
        (self.rule)(index, frame)
    }

    fn reset(&mut self) {
        self.log.lock().unwrap().push(Event::Reset);
        self.next_index = 0;
    }
}

pub fn frames_seen(log: &Log) -> Vec<Vec<f32>> {
    log.lock()
        .unwrap()
        .iter()
        .filter_map(|event| match event {
            Event::Frame(frame) => Some(frame.clone()),
            Event::Reset => None,
        })
        .collect()
}

/// Lets the scorer alone decide; tests override what they are about.
pub fn plain_config() -> VoiceDetectionConfig {
    VoiceDetectionConfig {
        speech_threshold: 0.5,
        silence_threshold: 0.5,
        min_loudness: 0.0,
        level_to: 0.0,
        level_window_ms: 0,
        min_speech_ms: 0,
        min_silence_ms: 0,
        pad_ms: 0,
    }
}

/// Uses a silent recording at the scorer's format, so frame `n` covers samples
/// `n * frame_len ..`.
pub fn detect_scripted(
    (rate, frame_len): (u32, usize),
    scores: &[f32],
    config: VoiceDetectionConfig,
) -> Vec<(usize, usize)> {
    bounds(&detect_scripted_spans((rate, frame_len), scores, config))
}

pub fn detect_scripted_spans(
    (rate, frame_len): (u32, usize),
    scores: &[f32],
    config: VoiceDetectionConfig,
) -> Vec<SpeechSpan> {
    let chunk = AudioChunk::new(vec![0.0; scores.len() * frame_len], rate, 1);
    let (scorer, _) = ScriptedScorer::from_scores(rate, frame_len, scores.to_vec());
    let mut detector =
        VoiceDetector::new(config, scorer, crate::support::resample_tuning()).unwrap();
    let spans = detector.detect(&chunk).unwrap();
    assert_spans_valid(&spans, &chunk);
    spans
}

pub fn bounds(spans: &[SpeechSpan]) -> Vec<(usize, usize)> {
    spans
        .iter()
        .map(|span| (span.start_frame, span.end_frame))
        .collect()
}

pub fn frame_ms((rate, frame_len): (u32, usize)) -> u32 {
    (frame_len as u64 * 1_000 / u64::from(rate)) as u32
}

pub fn frame_count(chunk: &AudioChunk) -> usize {
    chunk.samples.len() / usize::from(chunk.channels)
}

pub fn assert_spans_valid(spans: &[SpeechSpan], chunk: &AudioChunk) {
    let frames = frame_count(chunk);
    let mut previous_end = 0;
    for (position, span) in spans.iter().enumerate() {
        assert!(
            span.start_frame < span.end_frame,
            "span {position} is empty or reversed: {span:?}"
        );
        assert!(
            span.end_frame <= frames,
            "span {position} leaves the recording of {frames} frames: {span:?}"
        );
        assert!(
            position == 0 || span.start_frame >= previous_end,
            "span {position} overlaps the one before it or is out of order: {spans:?}"
        );
        assert_eq!(
            span.sample_rate_hz, chunk.sample_rate_hz,
            "span {position} is not at the recording's sample rate"
        );
        previous_end = span.end_frame;
    }
}

pub fn span(start_frame: usize, end_frame: usize, sample_rate_hz: u32) -> SpeechSpan {
    SpeechSpan {
        start_frame,
        end_frame,
        sample_rate_hz,
    }
}

pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

pub fn ms_to_frames(ms: u64, sample_rate_hz: u32) -> usize {
    (ms * u64::from(sample_rate_hz) / 1_000) as usize
}

pub fn interleave(mono: &[f32], sample_rate_hz: u32, channels: u16) -> AudioChunk {
    let mut samples = Vec::with_capacity(mono.len() * usize::from(channels));
    for value in mono {
        for _ in 0..channels {
            samples.push(*value);
        }
    }
    AudioChunk::new(samples, sample_rate_hz, channels)
}

pub fn silence(sample_rate_hz: u32, channels: u16, frames: usize) -> AudioChunk {
    interleave(&vec![0.0; frames], sample_rate_hz, channels)
}

/// Silence with a 440 Hz tone of amplitude 0.5 in each (start, end) millisecond burst.
pub fn bursts(
    sample_rate_hz: u32,
    channels: u16,
    total_ms: u64,
    bursts: &[(u64, u64)],
) -> AudioChunk {
    let mut mono = vec![0.0f32; ms_to_frames(total_ms, sample_rate_hz)];
    for (start_ms, end_ms) in bursts {
        let start = ms_to_frames(*start_ms, sample_rate_hz);
        let end = ms_to_frames(*end_ms, sample_rate_hz).min(mono.len());
        for (n, slot) in mono.iter_mut().enumerate().take(end).skip(start) {
            let phase = 2.0 * std::f32::consts::PI * 440.0 * n as f32 / sample_rate_hz as f32;
            *slot = 0.5 * phase.sin();
        }
    }
    interleave(&mono, sample_rate_hz, channels)
}

/// Every frame has a distinct non-zero value, so any frame can be told apart from inserted
/// silence.
pub fn ramp(sample_rate_hz: u32, channels: u16, frames: usize) -> AudioChunk {
    assert!(frames < 1_048_576, "the ramp would leave full scale");
    let mono: Vec<f32> = (0..frames).map(|n| (n + 1) as f32 / 1_048_576.0).collect();
    interleave(&mono, sample_rate_hz, channels)
}

pub fn ramp_value(n: usize) -> f32 {
    (n + 1) as f32 / 1_048_576.0
}

/// Flips every four samples, so it is equally loud everywhere.
pub fn square(frames: usize, amplitude: f32) -> Vec<f32> {
    (0..frames)
        .map(|n| {
            if (n / 4) % 2 == 0 {
                amplitude
            } else {
                -amplitude
            }
        })
        .collect()
}

/// Deterministic noise.
pub struct Noise(u64);

impl Noise {
    pub fn new(seed: u64) -> Self {
        Self(
            seed.wrapping_mul(2_862_933_555_777_941_757)
                .wrapping_add(3_037_000_493),
        )
    }

    pub fn unit(&mut self) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 40) as f32) / (1u64 << 24) as f32
    }

    pub fn sample(&mut self) -> f32 {
        self.unit() * 2.0 - 1.0
    }

    pub fn below(&mut self, limit: usize) -> usize {
        ((self.unit() * limit as f32) as usize).min(limit.saturating_sub(1))
    }
}

pub fn resample_tuning() -> audio::ResampleTuning {
    audio::ResampleTuning {
        block_frames: 1_024,
        min_sample_rate_hz: 800,
        max_sample_rate_hz: 384_000,
    }
}
