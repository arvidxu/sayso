use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{
    Data, Device, Error as DeviceError, FromSample, Host, I24, InputCallbackInfo, InterfaceType,
    SampleFormat, SizedSample, Stream, U24, default_host,
};
use dictation_api::{AudioCaptureSession, AudioSource};
use dictation_types::{AppError, AppResult, AudioCaptureConfig, AudioDevice, DeviceId, InputLevel};

use crate::capture_device::{Devices, Input, Opened, Recording, Shared, Trouble, Wanted};
use crate::microphone_session::MicrophoneSession;
use crate::{CaptureTuning, ResampleTuning};

#[derive(Debug, Clone)]
pub struct MicrophoneSource {
    resampling: ResampleTuning,
    pub(crate) capture: CaptureTuning,
}

impl MicrophoneSource {
    pub fn new(resampling: ResampleTuning, capture: CaptureTuning) -> AppResult<Self> {
        resampling.validate()?;
        capture.validate()?;
        Ok(Self {
            resampling,
            capture,
        })
    }
}

pub(crate) fn device_id(device: &Device) -> Option<String> {
    device
        .id()
        .ok()
        .map(|id| id.to_string())
        .filter(|id| !id.trim().is_empty())
}

/// Describes a device, or `None` when it cannot currently be recorded from.
fn describe(device: &Device, default_id: Option<&str>) -> Option<AudioDevice> {
    let id = device_id(device)?;
    let format = device.default_input_config().ok()?;
    let description = device.description().ok();
    let name = description
        .as_ref()
        .map_or_else(|| id.clone(), |d| d.name().to_owned());
    let interface = description
        .as_ref()
        .map_or(InterfaceType::Unknown, |d| d.interface_type());
    Some(AudioDevice {
        is_default: default_id == Some(id.as_str()),
        id: DeviceId::new(id),
        name,
        is_bluetooth: interface == InterfaceType::Bluetooth,
        is_built_in: interface == InterfaceType::BuiltIn,
        sample_rate_hz: format.sample_rate(),
        channels: format.channels(),
    })
}

fn find_device(host: &Host, wanted: Wanted<'_>) -> AppResult<Device> {
    let wanted = match wanted {
        Wanted::Listed(id) => DeviceId::new(id),
        Wanted::Default { not } => {
            return host
                .default_input_device()
                .filter(|device| not.is_none() || device_id(device).as_deref() != not)
                .ok_or_else(|| match not {
                    None => AppError::audio("no microphone is available"),
                    Some(_) => AppError::audio("no other microphone is available yet"),
                });
        }
    };
    let devices = host
        .input_devices()
        .map_err(|err| AppError::audio(format!("failed to list microphones: {err}")))?;
    for device in devices {
        // Compared as `DeviceId`s so an id reported with surrounding spaces still matches.
        if device_id(&device).map(DeviceId::new).as_ref() == Some(&wanted) {
            return Ok(device);
        }
    }
    Err(AppError::not_found(format!(
        "microphone '{wanted}' was not found"
    )))
}

/// Hands a delivery to the reader. Runs on the audio thread, so converting allocates nothing.
type Deliver = fn(&Data, &Shared);

/// How deliveries in `format` are read, or `None` when they cannot be.
fn deliver_for(format: SampleFormat) -> Option<Deliver> {
    fn typed<T>(data: &Data, shared: &Shared)
    where
        T: SizedSample,
        f32: FromSample<T>,
    {
        if let Some(samples) = data.as_slice::<T>() {
            shared.push(samples.iter().map(|sample| sample.to_sample::<f32>()));
        }
    }
    let deliver: Deliver = match format {
        SampleFormat::F32 => typed::<f32>,
        SampleFormat::F64 => typed::<f64>,
        SampleFormat::I8 => typed::<i8>,
        SampleFormat::I16 => typed::<i16>,
        SampleFormat::I24 => typed::<I24>,
        SampleFormat::I32 => typed::<i32>,
        SampleFormat::I64 => typed::<i64>,
        SampleFormat::U8 => typed::<u8>,
        SampleFormat::U16 => typed::<u16>,
        SampleFormat::U24 => typed::<U24>,
        SampleFormat::U32 => typed::<u32>,
        SampleFormat::U64 => typed::<u64>,
        _ => return None,
    };
    Some(deliver)
}

impl Recording for Stream {
    fn pause(&self) {
        // A stream that cannot be paused stops when it is dropped, which follows soon.
        StreamTrait::pause(self).ok();
    }
}

impl MicrophoneSource {
    fn open_device(
        &self,
        device: &Device,
        capture: &CaptureTuning,
        level: InputLevel,
    ) -> AppResult<Opened> {
        let format = device.default_input_config().map_err(|err| {
            AppError::audio(format!("failed to read the microphone's format: {err}"))
        })?;
        let sample_format = format.sample_format();
        let Some(deliver) = deliver_for(sample_format) else {
            return Err(AppError::unsupported(format!(
                "the microphone delivers samples as {sample_format}, which cannot be read"
            )));
        };
        let (rate, channels) = (format.sample_rate(), format.channels());
        let shared = Shared::new(rate, channels, capture, level)?;

        let (for_audio, for_errors) = (shared.clone(), shared.clone());
        let on_audio = move |data: &Data, _: &InputCallbackInfo| deliver(data, &for_audio);
        let on_error = move |err: DeviceError| for_errors.report(Trouble::of(err.kind()), &err);
        let stream = device
            .build_input_stream_raw(
                format.into(),
                sample_format,
                on_audio,
                on_error,
                Some(Duration::from_millis(capture.stream_timeout_ms)),
            )
            .map_err(|err| AppError::audio(format!("failed to open the microphone: {err}")))?;
        stream
            .play()
            .map_err(|err| AppError::audio(format!("failed to start recording: {err}")))?;
        Ok(Opened {
            input: Input {
                stream: Box::new(stream),
                shared,
                device_id: device_id(device),
            },
            sample_rate_hz: rate,
            channels,
        })
    }
}

impl Devices for MicrophoneSource {
    fn open(
        &self,
        wanted: Wanted<'_>,
        tuning: &CaptureTuning,
        level: InputLevel,
    ) -> AppResult<Opened> {
        let device = find_device(&default_host(), wanted)?;
        self.open_device(&device, tuning, level)
    }
}

impl AudioSource for MicrophoneSource {
    fn list_devices(&self) -> AppResult<Vec<AudioDevice>> {
        let host = default_host();
        let default_id = host.default_input_device().and_then(|d| device_id(&d));
        let devices = host
            .input_devices()
            .map_err(|err| AppError::audio(format!("failed to list microphones: {err}")))?;
        Ok(devices
            .filter_map(|device| describe(&device, default_id.as_deref()))
            .collect())
    }

    /// A chosen microphone falls back to the system default only when `config` asks for it;
    /// the default itself is always followed to whatever the system picks next.
    fn start_capture(&self, config: AudioCaptureConfig) -> AppResult<Box<dyn AudioCaptureSession>> {
        Ok(Box::new(MicrophoneSession::start(
            Box::new(self.clone()),
            self.capture.clone(),
            &self.resampling,
            &config,
        )?))
    }
}
