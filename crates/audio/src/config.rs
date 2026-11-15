use configuration::{PackageConfig, ProductConfig};
use dictation_types::{AppError, AppResult, UserSettings};
use serde::Deserialize;
use serde_json::from_str;

use crate::{CaptureTuning, ResampleTuning, SegmentationTuning, VoiceDetectionConfig};

pub const BUNDLED_CONFIG: &str = include_str!("../audio.conf.json");

#[derive(Debug, Clone, PartialEq)]
pub struct AudioConfig {
    pub voice_detection: VoiceDetectionConfig,
    pub capture: CaptureTuning,
    pub resampling: ResampleTuning,
    pub segmentation: SegmentationTuning,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConfigFile {
    voice_detection: VoiceDetectionFile,
    capture: CaptureFile,
    resampling: ResamplingFile,
    segmentation: SegmentationFile,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct VoiceDetectionFile {
    speech_threshold: f32,
    silence_threshold: f32,
    min_loudness: f32,
    level_to: f32,
    level_window_ms: u32,
    min_speech_ms: u32,
    min_silence_ms: u32,
    pad_ms: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CaptureFile {
    stream_timeout_ms: u64,
    device_switch_poll_ms: u64,
    reopen_interval_ms: u64,
    level_window_ms: u32,
    max_recording_ms: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ResamplingFile {
    block_frames: usize,
    min_sample_rate_hz: u32,
    max_sample_rate_hz: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SegmentationFile {
    gap_ms: u32,
    min_part_fraction_of_target: f32,
    quiet_window_ms: u32,
    target_fraction_of_max: f32,
}

fn in_group(group: &str, err: AppError) -> AppError {
    AppError::new(err.kind, format!("{group}: {}", err.message))
}

const MS_PER_SECOND: u64 = 1_000;

impl PackageConfig for AudioConfig {
    const NAME: &'static str = "audio";
    const BUNDLED: &'static str = BUNDLED_CONFIG;

    fn from_json(text: &str) -> AppResult<Self> {
        Self::from_json(text)
    }
}

impl AudioConfig {
    pub fn bundled() -> AppResult<Self> {
        Self::from_json(BUNDLED_CONFIG)
    }

    /// Every value must be present and valid; unknown keys are refused.
    pub fn from_json(text: &str) -> AppResult<Self> {
        let invalid =
            |err| AppError::validation(format!("the audio configuration is not valid: {err}"));
        let file: ConfigFile = from_str(text).map_err(invalid)?;
        let config = Self {
            voice_detection: VoiceDetectionConfig {
                speech_threshold: file.voice_detection.speech_threshold,
                silence_threshold: file.voice_detection.silence_threshold,
                min_loudness: file.voice_detection.min_loudness,
                level_to: file.voice_detection.level_to,
                level_window_ms: file.voice_detection.level_window_ms,
                min_speech_ms: file.voice_detection.min_speech_ms,
                min_silence_ms: file.voice_detection.min_silence_ms,
                pad_ms: file.voice_detection.pad_ms,
            },
            capture: CaptureTuning {
                stream_timeout_ms: file.capture.stream_timeout_ms,
                device_switch_poll_ms: file.capture.device_switch_poll_ms,
                reopen_interval_ms: file.capture.reopen_interval_ms,
                level_window_ms: file.capture.level_window_ms,
                max_recording_ms: file.capture.max_recording_ms,
            },
            resampling: ResampleTuning {
                block_frames: file.resampling.block_frames,
                min_sample_rate_hz: file.resampling.min_sample_rate_hz,
                max_sample_rate_hz: file.resampling.max_sample_rate_hz,
            },
            segmentation: SegmentationTuning {
                gap_ms: file.segmentation.gap_ms,
                min_part_fraction_of_target: file.segmentation.min_part_fraction_of_target,
                quiet_window_ms: file.segmentation.quiet_window_ms,
                target_fraction_of_max: file.segmentation.target_fraction_of_max,
            },
        };
        config.validate()?;
        Ok(config)
    }

    /// Also checks that no duration is longer than the longest recording.
    pub fn validate(&self) -> AppResult<()> {
        let detection = &self.voice_detection;
        detection
            .validate()
            .map_err(|err| in_group("voiceDetection", err))?;
        self.capture
            .validate()
            .map_err(|err| in_group("capture", err))?;
        self.resampling
            .validate()
            .map_err(|err| in_group("resampling", err))?;
        self.segmentation
            .validate()
            .map_err(|err| in_group("segmentation", err))?;
        let longest = self.capture.max_recording_ms;
        let durations = [
            ("voiceDetection.levelWindowMs", detection.level_window_ms),
            ("voiceDetection.minSpeechMs", detection.min_speech_ms),
            ("voiceDetection.minSilenceMs", detection.min_silence_ms),
            ("voiceDetection.padMs", detection.pad_ms),
            ("segmentation.gapMs", self.segmentation.gap_ms),
            (
                "segmentation.quietWindowMs",
                self.segmentation.quiet_window_ms,
            ),
        ];
        for (key, value) in durations {
            if u64::from(value) > longest {
                return Err(AppError::validation(format!(
                    "{key} must not be longer than capture.maxRecordingMs"
                )));
            }
        }
        Ok(())
    }

    /// The audio values of the product configuration, also checked against the longest
    /// recording a new user starts with.
    pub fn from_product(product: &ProductConfig) -> AppResult<Self> {
        let config: Self = product.package()?;
        if let Some(secs) = product.initial_settings.max_recording_secs {
            config
                .capture_lasting(secs)
                .map_err(|err| in_group("dictation.initialSettings.maxRecordingSecs", err))?;
        }
        Ok(config)
    }

    /// The capture tuning for a user with `settings`: one recording may last as long as the
    /// user chose, or as long as configured when the user has not chosen.
    pub fn capture_for(&self, settings: &UserSettings) -> AppResult<CaptureTuning> {
        let Some(secs) = settings.max_recording_secs else {
            return Ok(self.capture.clone());
        };
        self.capture_lasting(secs).map_err(|err| {
            AppError::new(
                err.kind,
                format!(
                    "a longest recording of {secs} s cannot be used: {}",
                    err.message
                ),
            )
        })
    }

    /// The capture tuning with recordings lasting up to `secs`, checked against the rest of
    /// the audio values.
    fn capture_lasting(&self, secs: u32) -> AppResult<CaptureTuning> {
        let mut audio = self.clone();
        audio.capture.max_recording_ms = u64::from(secs).saturating_mul(MS_PER_SECOND);
        audio.validate()?;
        Ok(audio.capture)
    }
}
