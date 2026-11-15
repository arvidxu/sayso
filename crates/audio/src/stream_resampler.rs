use std::fmt;
use std::mem;

use dictation_types::{AppError, AppResult};
use rubato::audioadapter_buffers::owned::InterleavedOwned;
use rubato::{Fft, FixedSync, Indexing, Resampler};

use crate::{ResampleTuning, downmix_to_mono};

/// Call [`flush`](Self::flush) once at the end; total output then has the input's duration.
/// Frames split between pushes are joined; only a trailing partial frame is dropped.
pub struct StreamResampler {
    input_sample_rate_hz: u32,
    input_channels: u16,
    target_sample_rate_hz: u32,
    resampler: Option<Fft<f32>>,
    pending_mono: Vec<f32>,
    /// The start of a frame whose other channels have not arrived yet.
    partial_frame: Vec<f32>,
    frames_to_trim: usize,
    total_input_frames: usize,
    total_output_frames: usize,
}

impl fmt::Debug for StreamResampler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StreamResampler")
            .field("input_sample_rate_hz", &self.input_sample_rate_hz)
            .field("input_channels", &self.input_channels)
            .field("target_sample_rate_hz", &self.target_sample_rate_hz)
            .field("pending_frames", &self.pending_mono.len())
            .finish()
    }
}

impl StreamResampler {
    pub fn new(
        input_sample_rate_hz: u32,
        input_channels: u16,
        target_sample_rate_hz: u32,
        tuning: &ResampleTuning,
    ) -> AppResult<Self> {
        tuning.validate()?;
        tuning.check_rates(input_sample_rate_hz, target_sample_rate_hz)?;
        if input_channels == 0 {
            return Err(AppError::validation(
                "audio channel count must be greater than zero",
            ));
        }

        let (resampler, frames_to_trim) = if input_sample_rate_hz == target_sample_rate_hz {
            (None, 0)
        } else {
            let fft = Fft::<f32>::new(
                input_sample_rate_hz as usize,
                target_sample_rate_hz as usize,
                tuning.block_frames,
                1,
                FixedSync::Input,
            )
            .map_err(|err| AppError::audio(format!("failed to set up the resampler: {err}")))?;
            let delay = fft.output_delay();
            (Some(fft), delay)
        };

        Ok(Self {
            input_sample_rate_hz,
            input_channels,
            target_sample_rate_hz,
            resampler,
            pending_mono: Vec::new(),
            partial_frame: Vec::new(),
            frames_to_trim,
            total_input_frames: 0,
            total_output_frames: 0,
        })
    }

    /// Drops the resampler's startup delay and clips: audio near full scale can overshoot.
    /// Resamples whole blocks of the pending audio from `taken` on, moving `taken` past each.
    fn resample_pending(&mut self, taken: &mut usize, produced: &mut Vec<f32>) -> AppResult<()> {
        while let Some(resampler) = &mut self.resampler {
            let needed = resampler.input_frames_next();
            if self.pending_mono.len() - *taken < needed {
                break;
            }
            let block = self.pending_mono[*taken..*taken + needed].to_vec();
            *taken += needed;
            let input = InterleavedOwned::new_from(block, 1, needed)
                .map_err(|err| AppError::audio(format!("failed to wrap an audio block: {err}")))?;
            let output = resampler
                .process(&input, None)
                .map_err(|err| AppError::audio(format!("failed to resample a block: {err}")))?;
            let ready = self.trim_delay(output.take_data());
            self.total_output_frames += ready.len();
            produced.extend_from_slice(&ready);
        }
        Ok(())
    }

    fn trim_delay(&mut self, mut samples: Vec<f32>) -> Vec<f32> {
        if self.frames_to_trim > 0 {
            let trim = self.frames_to_trim.min(samples.len());
            samples.drain(..trim);
            self.frames_to_trim -= trim;
        }
        for sample in &mut samples {
            *sample = sample.clamp(-1.0, 1.0);
        }
        samples
    }

    /// Some audio is held back until more arrives or [`flush`](Self::flush) is called.
    pub fn push_samples(&mut self, samples: &[f32]) -> AppResult<Vec<f32>> {
        let joined;
        let samples = if self.partial_frame.is_empty() {
            samples
        } else {
            self.partial_frame.extend_from_slice(samples);
            joined = mem::take(&mut self.partial_frame);
            &joined[..]
        };
        let whole = samples.len() - samples.len() % usize::from(self.input_channels);
        self.partial_frame.extend_from_slice(&samples[whole..]);
        let mono = downmix_to_mono(&samples[..whole], self.input_channels)?;
        if mono.is_empty() {
            return Ok(Vec::new());
        }
        self.total_input_frames += mono.len();
        if self.resampler.is_none() {
            self.total_output_frames += mono.len();
            return Ok(mono);
        }

        self.pending_mono.extend_from_slice(&mono);
        let mut produced = Vec::new();
        // Taken off the front once at the end: once per block would move the rest every time,
        // which grows with the square of a long recording handed over at once.
        let mut taken = 0;
        let resampled = self.resample_pending(&mut taken, &mut produced);
        self.pending_mono.drain(..taken);
        resampled?;
        Ok(produced)
    }

    /// Ends the stream; the resampler is then ready for a new one in the same format.
    pub fn flush(&mut self) -> AppResult<Vec<f32>> {
        if self.resampler.is_none() || self.total_input_frames == 0 {
            self.reset();
            return Ok(Vec::new());
        }
        // Integer maths so rounding never adds a frame.
        let expected_total = (u128::from(self.target_sample_rate_hz)
            * self.total_input_frames as u128)
            .div_ceil(u128::from(self.input_sample_rate_hz)) as usize;
        let still_owed = expected_total.saturating_sub(self.total_output_frames);

        let mut flushed = Vec::new();
        let mut pending = mem::take(&mut self.pending_mono);
        // The resampler can hold back a whole burst across several blocks, so an empty block
        // is not the end. This much silence always brings the tail out.
        let padding_limit = self.resampler.as_ref().map_or(0, |resampler| {
            let owed_output = still_owed
                .saturating_add(self.frames_to_trim)
                .saturating_add(resampler.output_frames_max());
            let input = (owed_output as u128 * u128::from(self.input_sample_rate_hz))
                .div_ceil(u128::from(self.target_sample_rate_hz));
            usize::try_from(input)
                .unwrap_or(usize::MAX)
                .saturating_add(resampler.input_frames_max())
        });
        let mut padding_fed = 0usize;
        while flushed.len() < still_owed {
            let Some(resampler) = &mut self.resampler else {
                break;
            };
            let needed = resampler.input_frames_next();
            let valid = pending.len().min(needed);
            let padding = needed - valid;
            if padding > 0 && padding_fed >= padding_limit {
                break;
            }
            padding_fed = padding_fed.saturating_add(padding);
            let mut block: Vec<f32> = pending.drain(..valid).collect();
            block.resize(needed, 0.0);
            let input = InterleavedOwned::new_from(block, 1, needed)
                .map_err(|err| AppError::audio(format!("failed to wrap the final block: {err}")))?;
            let indexing = Indexing::new().partial_len(valid);
            let output = resampler
                .process(&input, Some(&indexing))
                .map_err(|err| AppError::audio(format!("failed to resample the end: {err}")))?;
            let ready = self.trim_delay(output.take_data());
            flushed.extend_from_slice(&ready);
        }
        flushed.truncate(still_owed);
        self.reset();
        Ok(flushed)
    }

    pub fn reset(&mut self) {
        self.pending_mono.clear();
        self.partial_frame.clear();
        self.total_input_frames = 0;
        self.total_output_frames = 0;
        if let Some(resampler) = &mut self.resampler {
            resampler.reset();
            self.frames_to_trim = resampler.output_delay();
        }
    }
}
