#![allow(dead_code)]

use audio::{CaptureBuffer, CaptureTuning, DeviceInbox};
use dictation_types::{AudioChunk, InputLevel};

pub const DEVICE_RATES: [u32; 6] = [8_000, 16_000, 22_050, 44_100, 48_000, 96_000];

pub const DEVICE_CHANNELS: [u16; 2] = [1, 2];

pub fn buffer(device_hz: u32, channels: u16, target_hz: u32, chunk_ms: u32) -> CaptureBuffer {
    CaptureBuffer::new(
        device_hz,
        channels,
        target_hz,
        chunk_ms,
        &crate::support::resample_tuning(),
    )
    .unwrap_or_else(|err| {
        panic!("{device_hz} Hz x{channels} -> {target_hz} Hz, {chunk_ms} ms: {err:?}")
    })
}

/// The stream timeout plays no part in an inbox.
pub fn capture_tuning(level_window_ms: u32, max_recording_ms: u64) -> CaptureTuning {
    CaptureTuning {
        stream_timeout_ms: 1_000,
        device_switch_poll_ms: 10,
        reopen_interval_ms: 50,
        level_window_ms,
        max_recording_ms,
    }
}

pub fn inbox(device_hz: u32, channels: u16, tuning: &CaptureTuning) -> (DeviceInbox, InputLevel) {
    let level = InputLevel::new();
    let inbox = DeviceInbox::new(device_hz, channels, tuning, level.clone())
        .unwrap_or_else(|err| panic!("{device_hz} Hz x{channels}, {tuning:?}: {err:?}"));
    (inbox, level)
}

pub fn tone(sample_rate_hz: u32, frames: usize, hz: f64, amplitude: f64) -> Vec<f32> {
    (0..frames)
        .map(|n| {
            let phase = 2.0 * std::f64::consts::PI * hz * n as f64 / f64::from(sample_rate_hz);
            (amplitude * phase.sin()) as f32
        })
        .collect()
}

pub fn mix(a: &[f32], b: &[f32]) -> Vec<f32> {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).map(|(x, y)| x + y).collect()
}

/// Stereo channels differ (`mono + side`, `mono - side`) but average exactly to `mono`, so a
/// wrong mix-down shows.
pub fn device_audio(mono: &[f32], sample_rate_hz: u32, channels: u16) -> Vec<f32> {
    match channels {
        1 => mono.to_vec(),
        2 => {
            let side = tone(sample_rate_hz, mono.len(), 700.0, 0.25);
            let mut samples = Vec::with_capacity(mono.len() * 2);
            for (value, offset) in mono.iter().zip(&side) {
                samples.push(value + offset);
                samples.push(value - offset);
            }
            samples
        }
        other => panic!("no generated audio for {other} channels"),
    }
}

#[derive(Debug)]
pub struct Recording {
    pub chunks: Vec<AudioChunk>,
    pub rest: AudioChunk,
}

impl Recording {
    pub fn all_samples(&self) -> Vec<f32> {
        let mut samples = Vec::new();
        for chunk in &self.chunks {
            samples.extend_from_slice(&chunk.samples);
        }
        samples.extend_from_slice(&self.rest.samples);
        samples
    }
}

pub fn drain(buffer: &mut CaptureBuffer) -> Vec<AudioChunk> {
    let mut chunks = Vec::new();
    while let Some(chunk) = buffer.take_chunk() {
        chunks.push(chunk);
    }
    chunks
}

/// A block size of zero counts as one.
pub fn record(
    buffer: &mut CaptureBuffer,
    device_samples: &[f32],
    channels: u16,
    mut next_block_frames: impl FnMut() -> usize,
    take_while_recording: bool,
) -> Recording {
    let width = usize::from(channels);
    let mut chunks = Vec::new();
    let mut position = 0;
    while position < device_samples.len() {
        let size = next_block_frames().max(1).saturating_mul(width);
        let end = position.saturating_add(size).min(device_samples.len());
        buffer.push(&device_samples[position..end]).unwrap();
        position = end;
        if take_while_recording {
            chunks.extend(drain(buffer));
        }
    }
    let rest = buffer.finish().unwrap();
    Recording { chunks, rest }
}

pub fn expected_len(device_frames: usize, device_hz: u32, target_hz: u32) -> f64 {
    device_frames as f64 * f64::from(target_hz) / f64::from(device_hz)
}

pub fn middle(samples: &[f32]) -> &[f32] {
    &samples[samples.len() / 10..samples.len() * 9 / 10]
}

pub fn rms(samples: &[f32]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f64 = samples.iter().map(|s| f64::from(*s) * f64::from(*s)).sum();
    (sum / samples.len() as f64).sqrt()
}

pub fn amplitude_at(samples: &[f32], sample_rate_hz: u32, hz: f64) -> f64 {
    let (mut sin_sum, mut cos_sum) = (0.0f64, 0.0f64);
    for (n, sample) in samples.iter().enumerate() {
        let phase = 2.0 * std::f64::consts::PI * hz * n as f64 / f64::from(sample_rate_hz);
        sin_sum += f64::from(*sample) * phase.sin();
        cos_sum += f64::from(*sample) * phase.cos();
    }
    2.0 * (sin_sum * sin_sum + cos_sum * cos_sum).sqrt() / samples.len() as f64
}

pub fn assert_full_chunk(chunk: &AudioChunk, target_hz: u32, frames: usize, label: &str) {
    assert_eq!(chunk.channels, 1, "{label}: a chunk is not mono");
    assert_eq!(
        chunk.sample_rate_hz, target_hz,
        "{label}: a chunk is not at the target rate"
    );
    assert_eq!(
        chunk.samples.len(),
        frames,
        "{label}: a chunk does not hold exactly the chunk duration"
    );
}
