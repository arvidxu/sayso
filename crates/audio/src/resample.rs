use dictation_types::{AppResult, AudioChunk};

use crate::{ResampleTuning, StreamResampler};

/// Gives the same result as streaming the whole recording through a [`StreamResampler`].
pub fn resample_to_mono(
    chunk: &AudioChunk,
    target_sample_rate_hz: u32,
    tuning: &ResampleTuning,
) -> AppResult<AudioChunk> {
    let mut stream = StreamResampler::new(
        chunk.sample_rate_hz,
        chunk.channels,
        target_sample_rate_hz,
        tuning,
    )?;
    let mut samples = stream.push_samples(&chunk.samples)?;
    samples.extend(stream.flush()?);
    Ok(AudioChunk::new(samples, target_sample_rate_hz, 1))
}
