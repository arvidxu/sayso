use dictation_types::{AppError, AppResult};

use crate::SegmentationConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResampleTuning {
    /// Smaller blocks lower latency, larger ones cost less. Below 512 frames the block also sets
    /// the transform length, and with it the anti-aliasing cutoff.
    pub block_frames: usize,
    pub min_sample_rate_hz: u32,
    pub max_sample_rate_hz: u32,
}

impl ResampleTuning {
    pub fn validate(&self) -> AppResult<()> {
        if self.block_frames == 0 {
            return Err(AppError::validation(
                "block_frames must be greater than zero",
            ));
        }
        if self.min_sample_rate_hz == 0 || self.max_sample_rate_hz < self.min_sample_rate_hz {
            return Err(AppError::validation(
                "the sample rate range must start above zero and not end below its start",
            ));
        }
        if self.block_frames > self.max_sample_rate_hz as usize {
            return Err(AppError::validation(
                "block_frames must not hold more than one second of audio at the highest rate",
            ));
        }
        Ok(())
    }

    pub(crate) fn check_rates(&self, input_hz: u32, target_hz: u32) -> AppResult<()> {
        let range = self.min_sample_rate_hz..=self.max_sample_rate_hz;
        if !range.contains(&input_hz) || !range.contains(&target_hz) {
            return Err(AppError::validation(format!(
                "audio sample rates ({input_hz} Hz -> {target_hz} Hz) must be within {}..={} Hz",
                self.min_sample_rate_hz, self.max_sample_rate_hz
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureTuning {
    /// Also how long to wait for another microphone when the one in use goes away.
    pub stream_timeout_ms: u64,
    /// Only used while switching; shorter loses less audio.
    pub device_switch_poll_ms: u64,
    /// Keeps a device that stops right after reopening from being reopened in a tight loop.
    pub reopen_interval_ms: u64,
    pub level_window_ms: u32,
    /// Also bounds the memory a recording that nobody reads can take.
    pub max_recording_ms: u64,
}

impl CaptureTuning {
    pub fn validate(&self) -> AppResult<()> {
        let positive = [
            ("stream_timeout_ms", self.stream_timeout_ms),
            ("device_switch_poll_ms", self.device_switch_poll_ms),
            ("reopen_interval_ms", self.reopen_interval_ms),
            ("level_window_ms", u64::from(self.level_window_ms)),
            ("max_recording_ms", self.max_recording_ms),
        ];
        for (name, value) in positive {
            if value == 0 {
                return Err(AppError::validation(format!(
                    "{name} must be greater than zero"
                )));
            }
        }
        if self.device_switch_poll_ms > self.stream_timeout_ms {
            return Err(AppError::validation(
                "device_switch_poll_ms must not be longer than stream_timeout_ms",
            ));
        }
        if u64::from(self.level_window_ms) > self.max_recording_ms {
            return Err(AppError::validation(
                "level_window_ms must not be longer than max_recording_ms",
            ));
        }
        Ok(())
    }
}

/// The length limits are not here because they depend on the speech engine.
#[derive(Debug, Clone, PartialEq)]
pub struct SegmentationTuning {
    pub gap_ms: u32,
    /// Above 0, at most 1.
    pub min_part_fraction_of_target: f32,
    pub quiet_window_ms: u32,
    /// Used when an engine states only its maximum. Above 0, at most 1.
    pub target_fraction_of_max: f32,
}

impl SegmentationTuning {
    pub fn validate(&self) -> AppResult<()> {
        let fraction = self.min_part_fraction_of_target;
        if !(fraction > 0.0 && fraction <= 1.0) {
            return Err(AppError::validation(
                "min_part_fraction_of_target must be above 0 and at most 1",
            ));
        }
        let target = self.target_fraction_of_max;
        if !(target > 0.0 && target <= 1.0) {
            return Err(AppError::validation(
                "target_fraction_of_max must be above 0 and at most 1",
            ));
        }
        if self.quiet_window_ms == 0 {
            return Err(AppError::validation(
                "quiet_window_ms must be greater than zero",
            ));
        }
        Ok(())
    }

    pub fn for_max(&self, max_segment_ms: u64) -> SegmentationConfig {
        let target =
            (max_segment_ms as f64 * f64::from(self.target_fraction_of_max)).round() as u64;
        self.for_limits(target.clamp(1, max_segment_ms.max(1)), max_segment_ms)
    }

    fn for_limits(&self, target_segment_ms: u64, max_segment_ms: u64) -> SegmentationConfig {
        let min_part_ms =
            (target_segment_ms as f64 * f64::from(self.min_part_fraction_of_target)).round() as u64;
        SegmentationConfig {
            target_segment_ms,
            max_segment_ms,
            gap_ms: self.gap_ms,
            min_part_ms: min_part_ms.max(1),
            quiet_window_ms: self.quiet_window_ms,
        }
    }
}
