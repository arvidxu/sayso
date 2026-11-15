use dictation_types::{AppError, AppResult};

/// Averages each frame. Out-of-range samples are clipped, non-finite ones count as silence,
/// and a trailing partial frame is dropped.
pub fn downmix_to_mono(samples: &[f32], channels: u16) -> AppResult<Vec<f32>> {
    if channels == 0 {
        return Err(AppError::validation(
            "audio channel count must be greater than zero",
        ));
    }
    let finite = |sample: &f32| {
        if sample.is_finite() {
            sample.clamp(-1.0, 1.0)
        } else {
            0.0
        }
    };
    if channels == 1 {
        return Ok(samples.iter().map(finite).collect());
    }

    let width = usize::from(channels);
    let scale = 1.0 / f32::from(channels);
    Ok(samples
        .chunks_exact(width)
        .map(|frame| frame.iter().map(finite).sum::<f32>() * scale)
        .collect())
}
