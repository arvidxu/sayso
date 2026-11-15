use audio::loudness;

fn sine(amplitude: f32, hz: f32, sample_rate: u32, frames: usize) -> Vec<f32> {
    (0..frames)
        .map(|n| {
            amplitude * (2.0 * std::f32::consts::PI * hz * n as f32 / sample_rate as f32).sin()
        })
        .collect()
}

#[test]
fn empty_input_has_no_loudness() {
    assert_eq!(loudness(&[]), 0.0);
}

#[test]
fn silence_has_no_loudness() {
    for len in [1, 2, 160, 48_000] {
        assert_eq!(loudness(&vec![0.0; len]), 0.0, "{len} samples");
    }
    assert_eq!(loudness(&[0.0, -0.0, 0.0]), 0.0);
}

#[test]
fn a_full_scale_square_wave_has_loudness_one() {
    for len in [1usize, 2, 7, 256, 48_000] {
        let wave: Vec<f32> = (0..len)
            .map(|n| if n % 2 == 0 { 1.0 } else { -1.0 })
            .collect();
        let value = loudness(&wave);
        assert!((value - 1.0).abs() < 1e-4, "{len} samples: {value}");
    }
    assert!((loudness(&[1.0; 4_096]) - 1.0).abs() < 1e-4);
    assert!((loudness(&[-1.0; 4_096]) - 1.0).abs() < 1e-4);
}

#[test]
fn a_sine_is_as_loud_as_its_amplitude_over_root_two() {
    for amplitude in [0.01f32, 0.1, 0.25, 0.5, 0.9, 1.0] {
        for (hz, rate) in [(100.0, 8_000), (440.0, 16_000), (1_000.0, 48_000)] {
            // One second holds a whole number of periods of each tone.
            let value = loudness(&sine(amplitude, hz, rate, rate as usize));
            let expected = amplitude / std::f32::consts::SQRT_2;
            assert!(
                (value - expected).abs() <= expected * 0.01,
                "amplitude {amplitude}, {hz} Hz at {rate} Hz: {value} instead of {expected}"
            );
        }
    }
}

#[test]
fn loudness_follows_the_signal_level() {
    let quiet = loudness(&sine(0.1, 440.0, 16_000, 16_000));
    let loud = loudness(&sine(0.8, 440.0, 16_000, 16_000));
    assert!(quiet < loud);
    assert!((loud / quiet - 8.0).abs() < 0.05, "{loud} / {quiet}");
}

#[test]
fn a_constant_level_is_its_own_loudness() {
    for level in [0.001f32, 0.25, -0.25, 0.5, -0.75] {
        let value = loudness(&[level; 1_000]);
        assert!((value - level.abs()).abs() < 1e-4, "{level}: {value}");
    }
}

#[test]
fn loudness_does_not_depend_on_how_long_the_signal_is() {
    let short = loudness(&sine(0.5, 1_000.0, 16_000, 160));
    let long = loudness(&sine(0.5, 1_000.0, 16_000, 160_000));
    assert!((short - long).abs() < 1e-3, "{short} / {long}");
}
