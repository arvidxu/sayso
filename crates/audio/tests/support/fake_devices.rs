#![allow(dead_code)]

use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::time::Duration;

use cpal::ErrorKind as DeviceErrorKind;
use dictation_api::AudioCaptureSession;
use dictation_types::{AppError, AppResult, AudioCaptureConfig, DeviceId, InputLevel};

use audio::capture_device::{Devices, Input, Opened, Recording, Shared, Trouble, Wanted};
use audio::{CaptureTuning, MicrophoneSession, ResampleTuning};

/// Devices record at the requested rate by default, so samples come out as they went in.
pub const RATE_HZ: u32 = 16_000;
pub const CHUNK_MS: u32 = 10;
pub const CHUNK: usize = 160;
pub const SPEAKER: &str = "usb-speaker-mic";
pub const HEADSET: &str = "headset";

/// While it exists its device counts as in use.
struct Still(#[allow(dead_code)] Arc<()>);

impl Recording for Still {
    fn pause(&self) {}
}

struct Listed {
    id: &'static str,
    rate: u32,
    channels: u16,
}

#[derive(Default)]
struct Script {
    listed: Vec<Listed>,
    default: Option<&'static str>,
    failing_opens: usize,
    exclusive: bool,
    stops_on_open: Option<DeviceErrorKind>,
    asked: Vec<String>,
    opened: Vec<(&'static str, Arc<Shared>, Weak<()>)>,
}

/// Copies share one script.
#[derive(Clone, Default)]
pub struct FakeDevices(Arc<Mutex<Script>>);

impl FakeDevices {
    pub fn with(listed: &[&'static str], default: Option<&'static str>) -> Self {
        let devices = Self::default();
        for id in listed {
            devices.plug_in(id, RATE_HZ, 1);
        }
        devices.script().default = default;
        devices
    }

    fn script(&self) -> MutexGuard<'_, Script> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn plug_in(&self, id: &'static str, rate: u32, channels: u16) {
        let mut script = self.script();
        script.listed.retain(|device| device.id != id);
        script.listed.push(Listed { id, rate, channels });
    }

    pub fn unplug(&self, id: &str) {
        let mut script = self.script();
        script.listed.retain(|device| device.id != id);
        if script.default == Some(id) {
            script.default = None;
        }
    }

    pub fn set_default(&self, id: Option<&'static str>) {
        self.script().default = id;
    }

    pub fn fail_next_opens(&self, count: usize) {
        self.script().failing_opens = count;
    }

    pub fn asked(&self) -> Vec<String> {
        self.script().asked.clone()
    }

    pub fn opened(&self) -> Vec<&'static str> {
        self.script().opened.iter().map(|(id, ..)| *id).collect()
    }

    pub fn current(&self) -> Arc<Shared> {
        Arc::clone(&self.script().opened.last().expect("a stream was opened").1)
    }

    pub fn make_exclusive(&self) {
        self.script().exclusive = true;
    }

    pub fn stop_on_open(&self, kind: DeviceErrorKind) {
        self.script().stops_on_open = Some(kind);
    }

    pub fn deliver(&self, samples: &[f32]) {
        self.current().push(samples.iter().copied());
    }

    pub fn fail(&self, kind: DeviceErrorKind) {
        self.current()
            .report(Trouble::of(kind), format!("{kind:?}"));
    }
}

impl Devices for FakeDevices {
    fn open(
        &self,
        wanted: Wanted<'_>,
        tuning: &CaptureTuning,
        level: InputLevel,
    ) -> AppResult<Opened> {
        let mut script = self.script();
        let id = match wanted {
            Wanted::Listed(id) => {
                script.asked.push(id.to_owned());
                id.to_owned()
            }
            Wanted::Default { not } => {
                script.asked.push("default".to_owned());
                match script.default {
                    Some(id) if Some(id) != not => id.to_owned(),
                    _ => return Err(AppError::audio("no other microphone is available yet")),
                }
            }
        };
        let Some(device) = script.listed.iter().find(|device| device.id == id) else {
            return Err(AppError::not_found(format!(
                "microphone '{id}' was not found"
            )));
        };
        let (device_id, rate, channels) = (device.id, device.rate, device.channels);
        if script.failing_opens > 0 {
            script.failing_opens -= 1;
            return Err(AppError::audio("the device is still being set up"));
        }
        let in_use = script
            .opened
            .iter()
            .any(|(id, _, alive)| *id == device_id && alive.strong_count() > 0);
        if script.exclusive && in_use {
            return Err(AppError::audio("the device is busy"));
        }
        let shared = Shared::new(rate, channels, tuning, level)?;
        let alive = Arc::new(());
        script
            .opened
            .push((device_id, Arc::clone(&shared), Arc::downgrade(&alive)));
        if let Some(kind) = script.stops_on_open {
            shared.report(Trouble::of(kind), format!("{kind:?}"));
        }
        Ok(Opened {
            input: Input {
                stream: Box::new(Still(alive)),
                shared,
                device_id: Some(device_id.to_owned()),
            },
            sample_rate_hz: rate,
            channels,
        })
    }
}

pub const PATIENCE_MS: u64 = 300;
pub const REOPEN_MS: u64 = 30;
/// Well within the patience, so one read never outlasts it.
pub const READ_MS: u64 = 100;

pub fn capture() -> CaptureTuning {
    CaptureTuning {
        stream_timeout_ms: PATIENCE_MS,
        device_switch_poll_ms: 2,
        reopen_interval_ms: REOPEN_MS,
        level_window_ms: 10,
        max_recording_ms: 60_000,
    }
}

pub fn resampling() -> ResampleTuning {
    ResampleTuning {
        block_frames: 256,
        min_sample_rate_hz: 8_000,
        max_sample_rate_hz: 96_000,
    }
}

pub fn config(device: Option<&str>, fall_back_to_default: bool) -> AudioCaptureConfig {
    let mut config = AudioCaptureConfig::new(RATE_HZ, CHUNK_MS);
    config.device_id = device.map(DeviceId::new);
    config.fall_back_to_default = fall_back_to_default;
    config
}

pub fn start(devices: &FakeDevices, config: &AudioCaptureConfig) -> MicrophoneSession {
    MicrophoneSession::start(Box::new(devices.clone()), capture(), &resampling(), config)
        .expect("the recording starts")
}

pub fn chunk_of(value: f32) -> Vec<f32> {
    vec![value; CHUNK]
}

pub fn read(session: &mut MicrophoneSession) -> Option<Vec<f32>> {
    session
        .read_chunk(Duration::from_millis(READ_MS))
        .expect("reading works")
        .map(|chunk| chunk.samples)
}
