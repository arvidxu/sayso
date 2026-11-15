/// Root mean square of samples between -1 and 1, so from 0 to 1.
pub fn loudness(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f32 = samples.iter().map(|sample| sample * sample).sum();
    (sum / samples.len() as f32).sqrt()
}

/// Rounded down; 0 for a rate of 0.
pub(crate) fn frames_to_ms(frames: usize, sample_rate_hz: u32) -> u64 {
    if sample_rate_hz == 0 {
        return 0;
    }
    (frames as u64).saturating_mul(1_000) / u64::from(sample_rate_hz)
}

pub(crate) fn ms_to_frames(ms: u64, sample_rate_hz: u32) -> usize {
    (ms.saturating_mul(u64::from(sample_rate_hz)) / 1_000) as usize
}
