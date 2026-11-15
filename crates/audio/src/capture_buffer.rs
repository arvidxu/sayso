use std::collections::VecDeque;

use dictation_types::{AppError, AppResult, AudioChunk};

use crate::measure::{frames_to_ms, ms_to_frames};
use crate::{ResampleTuning, StreamResampler};

/// Every sample pushed comes out exactly once, in a chunk or in [`finish`](Self::finish).
/// Chunk sizes stay regular across [`switch_device`](Self::switch_device).
#[derive(Debug)]
pub struct CaptureBuffer {
    resampler: StreamResampler,
    resampling: ResampleTuning,
    sample_rate_hz: u32,
    converted_frames: usize,
    chunk_frames: usize,
    pending: Vec<f32>,
    ready: VecDeque<AudioChunk>,
    finished: bool,
}

impl CaptureBuffer {
    /// Chunk length is rounded down to whole samples, at least one.
    pub fn new(
        device_sample_rate_hz: u32,
        device_channels: u16,
        sample_rate_hz: u32,
        frame_duration_ms: u32,
        resampling: &ResampleTuning,
    ) -> AppResult<Self> {
        Self::check_output(sample_rate_hz, frame_duration_ms, resampling)?;
        Ok(Self {
            resampler: StreamResampler::new(
                device_sample_rate_hz,
                device_channels,
                sample_rate_hz,
                resampling,
            )?,
            resampling: resampling.clone(),
            sample_rate_hz,
            converted_frames: 0,
            chunk_frames: ms_to_frames(u64::from(frame_duration_ms), sample_rate_hz).max(1),
            pending: Vec::new(),
            ready: VecDeque::new(),
            finished: false,
        })
    }

    /// Usable before any device is opened, so a bad request never turns a microphone on.
    pub fn check_output(
        sample_rate_hz: u32,
        frame_duration_ms: u32,
        resampling: &ResampleTuning,
    ) -> AppResult<()> {
        if frame_duration_ms == 0 {
            return Err(AppError::validation(
                "frame_duration_ms must be greater than zero",
            ));
        }
        resampling.check_rates(sample_rate_hz, sample_rate_hz)
    }

    fn chunk(&self, samples: Vec<f32>) -> AudioChunk {
        AudioChunk::new(samples, self.sample_rate_hz, 1)
    }

    fn ended() -> AppError {
        AppError::audio("the recording has already ended")
    }

    /// Out-of-range samples are clipped and non-finite ones count as silence.
    pub fn push(&mut self, device_samples: &[f32]) -> AppResult<()> {
        if self.finished {
            return Err(Self::ended());
        }
        if device_samples.is_empty() {
            return Ok(());
        }
        let converted = self.resampler.push_samples(device_samples)?;
        self.accept(&converted);
        Ok(())
    }

    fn accept(&mut self, converted: &[f32]) {
        self.converted_frames += converted.len();
        self.pending.extend_from_slice(converted);
        // Taken off the front once at the end: once per chunk would move the rest every time,
        // which grows with the square of a long recording handed over at once.
        let mut taken = 0;
        while self.pending.len() - taken >= self.chunk_frames {
            let samples = self.pending[taken..taken + self.chunk_frames].to_vec();
            taken += self.chunk_frames;
            let chunk = self.chunk(samples);
            self.ready.push_back(chunk);
        }
        self.pending.drain(..taken);
    }

    /// Flushes the previous device's audio first so none of it is lost.
    pub fn switch_device(
        &mut self,
        device_sample_rate_hz: u32,
        device_channels: u16,
    ) -> AppResult<()> {
        if self.finished {
            return Err(Self::ended());
        }
        let next = StreamResampler::new(
            device_sample_rate_hz,
            device_channels,
            self.sample_rate_hz,
            &self.resampling,
        )?;
        let tail = self.resampler.flush()?;
        self.accept(&tail);
        self.resampler = next;
        Ok(())
    }

    /// Includes audio already taken.
    pub fn converted_ms(&self) -> u64 {
        frames_to_ms(self.converted_frames, self.sample_rate_hz)
    }

    pub fn take_chunk(&mut self) -> Option<AudioChunk> {
        if self.finished {
            return None;
        }
        self.ready.pop_front()
    }

    /// Can be called once.
    pub fn finish(&mut self) -> AppResult<AudioChunk> {
        if self.finished {
            return Err(Self::ended());
        }
        // Flushed first so a failure leaves the untaken audio in place.
        let tail = self.resampler.flush()?;
        self.finished = true;
        let mut unread = Vec::new();
        for chunk in self.ready.drain(..) {
            unread.extend_from_slice(&chunk.samples);
        }
        unread.append(&mut self.pending);
        unread.extend(tail);
        Ok(self.chunk(unread))
    }

    pub fn discard(&mut self) {
        self.finished = true;
        self.pending.clear();
        self.ready.clear();
        self.resampler.reset();
    }
}
