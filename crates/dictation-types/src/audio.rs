use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::DeviceId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioDevice {
    pub id: DeviceId,
    pub name: String,
    pub is_default: bool,
    pub is_bluetooth: bool,
    pub is_built_in: bool,
    pub sample_rate_hz: u32,
    pub channels: u16,
}

impl AudioDevice {
    pub fn new(id: DeviceId, name: impl Into<String>, sample_rate_hz: u32, channels: u16) -> Self {
        Self {
            id,
            name: name.into(),
            is_default: false,
            is_bluetooth: false,
            is_built_in: false,
            sample_rate_hz,
            channels,
        }
    }
}

/// Captured audio is always mono; the sample rate is whatever the consumer needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioCaptureConfig {
    /// `None` records from the system default.
    pub device_id: Option<DeviceId>,
    /// If the device goes missing, at the start or mid-recording: `true` switches to the
    /// default, `false` fails. Ignored without a `device_id`.
    pub fall_back_to_default: bool,
    pub sample_rate_hz: u32,
    pub frame_duration_ms: u32,
}

impl AudioCaptureConfig {
    /// Default device; a device set later is not replaced when gone unless
    /// `fall_back_to_default` is set.
    pub fn new(sample_rate_hz: u32, frame_duration_ms: u32) -> Self {
        Self {
            device_id: None,
            fall_back_to_default: false,
            sample_rate_hz,
            frame_duration_ms,
        }
    }
}

/// Input loudness, 0 to 1; clones share it and read it from any thread without blocking.
#[derive(Debug, Clone, Default)]
pub struct InputLevel(Arc<AtomicU32>);

impl InputLevel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }

    /// Clamps to 0..=1, infinities included; NaN counts as 0.
    pub fn set(&self, level: f32) {
        let level = if level.is_nan() {
            0.0
        } else {
            level.clamp(0.0, 1.0)
        };
        self.0.store(level.to_bits(), Ordering::Relaxed);
    }
}

/// PCM samples interleaved by channel, each between -1 and 1.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioChunk {
    pub samples: Vec<f32>,
    pub sample_rate_hz: u32,
    pub channels: u16,
}

impl AudioChunk {
    pub fn new(samples: Vec<f32>, sample_rate_hz: u32, channels: u16) -> Self {
        Self {
            samples,
            sample_rate_hz,
            channels,
        }
    }

    /// Zero when the format is not usable.
    pub fn duration_ms(&self) -> u64 {
        if self.sample_rate_hz == 0 || self.channels == 0 {
            return 0;
        }
        let frames = self.samples.len() as u64 / u64::from(self.channels);
        (frames * 1_000) / u64::from(self.sample_rate_hz)
    }
}
