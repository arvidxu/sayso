//! Only stores and meters what arrives, so the device's audio thread never waits on conversion.

use std::mem;

use dictation_types::{AppError, AppResult, InputLevel};

use crate::CaptureTuning;
use crate::measure::ms_to_frames;

/// Loudness of the most recent window, kept in a ring allocated once.
#[derive(Debug)]
struct LevelMeter {
    level: InputLevel,
    squares: Vec<f32>,
    next: usize,
    filled: usize,
    sum: f64,
}

impl LevelMeter {
    fn new(level: InputLevel, window_samples: usize) -> Self {
        Self {
            level,
            squares: vec![0.0; window_samples.max(1)],
            next: 0,
            filled: 0,
            sum: 0.0,
        }
    }

    fn add(&mut self, sample: f32) {
        let square = sample * sample;
        self.sum += f64::from(square) - f64::from(self.squares[self.next]);
        self.squares[self.next] = square;
        self.next = (self.next + 1) % self.squares.len();
        if self.next == 0 {
            // Recompute exactly each lap so rounding errors cannot build up.
            self.sum = self.squares.iter().map(|s| f64::from(*s)).sum();
        }
        self.filled = (self.filled + 1).min(self.squares.len());
        self.level
            .set((self.sum.max(0.0) / self.filled as f64).sqrt() as f32);
    }

    fn silence(&mut self) {
        self.squares.fill(0.0);
        self.next = 0;
        self.filled = 0;
        self.sum = 0.0;
        self.level.set(0.0);
    }
}

/// Interleaved samples in the device's format, not yet converted. Audio received before an
/// interruption is kept.
#[derive(Debug)]
pub struct DeviceInbox {
    channels: usize,
    samples: Vec<f32>,
    max_samples: usize,
    received_samples: usize,
    meter: LevelMeter,
    dropouts: u32,
    interruption: Option<AppError>,
    max_recording_ms: u64,
}

impl DeviceInbox {
    /// `tuning` has already been checked by whoever made it.
    pub fn new(
        sample_rate_hz: u32,
        channels: u16,
        tuning: &CaptureTuning,
        level: InputLevel,
    ) -> AppResult<Self> {
        if sample_rate_hz == 0 || channels == 0 {
            return Err(AppError::validation(
                "a device must have a sample rate and at least one channel",
            ));
        }
        let channels = usize::from(channels);
        let frames = |ms: u64| ms_to_frames(ms, sample_rate_hz).max(1);
        level.set(0.0);
        Ok(Self {
            channels,
            samples: Vec::new(),
            max_samples: frames(tuning.max_recording_ms).saturating_mul(channels),
            received_samples: 0,
            meter: LevelMeter::new(
                level,
                frames(u64::from(tuning.level_window_ms)).saturating_mul(channels),
            ),
            dropouts: 0,
            interruption: None,
            max_recording_ms: tuning.max_recording_ms,
        })
    }

    /// Out-of-range samples are clipped and non-finite ones count as silence. Ignored once
    /// interrupted.
    pub fn push(&mut self, samples: impl IntoIterator<Item = f32>) {
        if self.interruption.is_some() {
            return;
        }
        for sample in samples {
            let sample = if sample.is_finite() {
                sample.clamp(-1.0, 1.0)
            } else {
                0.0
            };
            self.samples.push(sample);
            self.received_samples += 1;
            self.meter.add(sample);
            if self.received_samples >= self.max_samples {
                self.interrupt(AppError::timeout(format!(
                    "the recording reached its maximum length of {} ms",
                    self.max_recording_ms
                )));
                return;
            }
        }
    }

    pub fn note_dropout(&mut self) {
        self.dropouts = self.dropouts.saturating_add(1);
    }

    /// Only the first interruption is kept.
    pub fn interrupt(&mut self, reason: AppError) {
        if self.interruption.is_none() {
            self.interruption = Some(reason);
            self.meter.silence();
        }
    }

    /// Allocates the next buffer here on the reader's side, so the audio thread only allocates
    /// when a delivery is larger than any before.
    pub fn take_samples(&mut self) -> Vec<f32> {
        let whole = self.samples.len() - self.samples.len() % self.channels;
        let mut next = Vec::with_capacity(self.samples.capacity());
        // A frame split across deliveries stays until the rest of it arrives.
        next.extend_from_slice(&self.samples[whole..]);
        self.samples.truncate(whole);
        mem::replace(&mut self.samples, next)
    }

    pub fn has_samples(&self) -> bool {
        self.samples.len() >= self.channels
    }

    pub fn interruption(&self) -> Option<&AppError> {
        self.interruption.as_ref()
    }

    pub fn is_receiving(&self) -> bool {
        self.interruption.is_none()
    }

    pub fn dropouts(&self) -> u32 {
        self.dropouts
    }
}
