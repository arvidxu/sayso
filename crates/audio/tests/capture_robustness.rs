#[path = "support/capture.rs"]
mod capture;
mod support;

use audio::CaptureBuffer;
use capture::{buffer, device_audio, drain, record, tone};
use dictation_types::{AppResult, ErrorKind};
use support::resample_tuning;

fn build(device_hz: u32, channels: u16, target_hz: u32, chunk_ms: u32) -> AppResult<CaptureBuffer> {
    CaptureBuffer::new(device_hz, channels, target_hz, chunk_ms, &resample_tuning())
}

fn assert_rejected(device_hz: u32, channels: u16, target_hz: u32, chunk_ms: u32) {
    match build(device_hz, channels, target_hz, chunk_ms) {
        Ok(_) => panic!(
            "a buffer was built for {device_hz} Hz x{channels} -> {target_hz} Hz, {chunk_ms} ms"
        ),
        Err(err) => assert_eq!(
            err.kind,
            ErrorKind::Validation,
            "{device_hz} Hz x{channels} -> {target_hz} Hz, {chunk_ms} ms: {err:?}"
        ),
    }
}

#[test]
fn non_finite_samples_count_as_silence() {
    let mut capture = buffer(16_000, 1, 16_000, 20);
    capture
        .push(&[0.5, f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.25])
        .unwrap();
    assert_eq!(
        capture.finish().unwrap().samples,
        [0.5, 0.0, 0.0, 0.0, -0.25]
    );

    let mut capture = buffer(16_000, 2, 16_000, 20);
    capture
        .push(&[
            f32::NAN,
            0.5,
            0.25,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NAN,
        ])
        .unwrap();
    assert_eq!(capture.finish().unwrap().samples, [0.25, 0.125, 0.0]);
}

#[test]
fn non_finite_samples_convert_like_silence_in_their_place() {
    for (device_hz, channels, target_hz) in [(48_000u32, 2u16, 16_000u32), (8_000, 1, 16_000)] {
        let label = format!("{device_hz} Hz x{channels} -> {target_hz} Hz");
        let frames = device_hz as usize / 2;
        let clean = device_audio(&tone(device_hz, frames, 300.0, 0.5), device_hz, channels);
        let mut silenced = clean.clone();
        let mut broken = clean.clone();
        for index in (0..clean.len()).step_by(13) {
            silenced[index] = 0.0;
            broken[index] = match index % 3 {
                0 => f32::NAN,
                1 => f32::INFINITY,
                _ => f32::NEG_INFINITY,
            };
        }
        let mut expected = buffer(device_hz, channels, target_hz, 20);
        let expected = record(&mut expected, &silenced, channels, || 500, true).all_samples();
        let mut actual = buffer(device_hz, channels, target_hz, 20);
        let actual = record(&mut actual, &broken, channels, || 500, true).all_samples();
        assert!(
            actual.iter().all(|s| in_range(*s)),
            "{label}: non-finite or out-of-range output"
        );
        assert_eq!(actual, expected, "{label}");
    }
}

#[test]
fn a_recording_of_only_non_finite_samples_is_silence_of_the_same_duration() {
    for (device_hz, channels, target_hz) in [(16_000u32, 1u16, 16_000u32), (48_000, 2, 16_000)] {
        let frames = device_hz as usize / 2;
        let input = vec![f32::NAN; frames * usize::from(channels)];
        let mut capture = buffer(device_hz, channels, target_hz, 20);
        let output = record(&mut capture, &input, channels, || 480, true).all_samples();
        assert!(
            output.len().abs_diff(8_000) <= 3,
            "{} samples",
            output.len()
        );
        assert!(output.iter().all(|s| *s == 0.0));
    }
}

#[test]
fn an_empty_block_is_ignored() {
    for (device_hz, channels, target_hz) in [(16_000u32, 1u16, 16_000u32), (48_000, 2, 16_000)] {
        let frames = device_hz as usize / 2;
        let input = device_audio(&tone(device_hz, frames, 300.0, 0.5), device_hz, channels);
        let mut plain = buffer(device_hz, channels, target_hz, 20);
        let mut with_empties = buffer(device_hz, channels, target_hz, 20);
        with_empties.push(&[]).unwrap();
        assert_eq!(with_empties.take_chunk(), None);
        let mut plain_out = Vec::new();
        let mut empties_out = Vec::new();
        for block in input.chunks(441 * usize::from(channels)) {
            plain.push(block).unwrap();
            with_empties.push(&[]).unwrap();
            with_empties.push(block).unwrap();
            with_empties.push(&[]).unwrap();
            plain_out.extend(drain(&mut plain));
            empties_out.extend(drain(&mut with_empties));
        }
        assert_eq!(empties_out, plain_out);
        assert_eq!(with_empties.finish().unwrap(), plain.finish().unwrap());
    }
}

#[test]
fn a_block_that_is_not_a_whole_number_of_frames_does_not_panic() {
    for target_hz in [16_000u32, 48_000] {
        for len in [1usize, 3, 5, 959, 961, 2_049] {
            let mut capture = buffer(48_000, 2, target_hz, 20);
            let block = vec![0.25f32; len];
            for _ in 0..40 {
                capture.push(&block).unwrap();
                while let Some(chunk) = capture.take_chunk() {
                    assert!(chunk.samples.iter().all(|s| in_range(*s)));
                }
            }
            let rest = capture.finish().unwrap();
            assert!(rest.samples.iter().all(|s| in_range(*s)));
        }
    }
}

#[test]
fn a_frame_split_between_two_blocks_is_recorded_once() {
    let input = device_audio(&tone(16_000, 4_000, 300.0, 0.5), 16_000, 2);
    let mut capture = buffer(16_000, 2, 16_000, 20);
    capture.push(&input[..3]).unwrap();
    capture.push(&input[3..]).unwrap();
    let output = capture.finish().unwrap().samples;
    let expected = tone(16_000, 4_000, 300.0, 0.5);
    assert_eq!(output.len(), expected.len());
    let worst = output
        .iter()
        .zip(&expected)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f32::max);
    assert!(worst < 1e-6, "the recording is off by up to {worst}");
}

/// Even where a conversion would overshoot at sharp edges.
fn assert_in_range(samples: &[f32], label: &str) {
    assert!(!samples.is_empty(), "{label}: nothing came out");
    for (n, sample) in samples.iter().enumerate() {
        assert!(in_range(*sample), "{label}: sample {n} is {sample}");
    }
}

fn in_range(sample: f32) -> bool {
    sample.is_finite() && (-1.0..=1.0).contains(&sample)
}

const EXTREME_FORMATS: [(u32, u16, u32); 6] = [
    (16_000, 1, 16_000),
    (16_000, 2, 16_000),
    (48_000, 1, 16_000),
    (48_000, 2, 16_000),
    (8_000, 1, 16_000),
    (22_050, 2, 48_000),
];

#[test]
fn the_most_extreme_samples_never_panic_or_give_non_finite_audio() {
    let extremes = [
        f32::MAX,
        f32::MIN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
        -f32::NAN,
        f32::MIN_POSITIVE,
        -f32::MIN_POSITIVE,
        1.0e-45,
    ];
    for (device_hz, channels, target_hz) in EXTREME_FORMATS {
        let label = format!("{device_hz} Hz x{channels} -> {target_hz} Hz");
        let frames = device_hz as usize / 4;
        let input: Vec<f32> = (0..frames * usize::from(channels))
            .map(|n| extremes[(n / 3) % extremes.len()])
            .collect();
        let mut capture = buffer(device_hz, channels, target_hz, 20);
        let recording = record(&mut capture, &input, channels, || 480, true);
        assert_in_range(&recording.all_samples(), &label);
    }
}

#[test]
fn the_largest_finite_samples_come_out_as_full_scale() {
    for channels in [1u16, 2] {
        let mut capture = buffer(16_000, channels, 16_000, 20);
        let mut input = vec![f32::MAX; 10 * usize::from(channels)];
        input.extend(vec![f32::MIN; 10 * usize::from(channels)]);
        capture.push(&input).unwrap();
        let mut expected = vec![1.0f32; 10];
        expected.extend([-1.0f32; 10]);
        assert_eq!(
            capture.finish().unwrap().samples,
            expected,
            "{channels} channels"
        );
    }
}

#[test]
fn a_stereo_frame_of_two_full_scale_samples_does_not_overflow() {
    let mut capture = buffer(16_000, 2, 16_000, 20);
    capture
        .push(&[
            1.0,
            1.0,
            -1.0,
            -1.0,
            f32::MAX,
            f32::MAX,
            f32::MIN,
            f32::MIN,
            1.0,
            -1.0,
        ])
        .unwrap();
    assert_eq!(
        capture.finish().unwrap().samples,
        [1.0, -1.0, 1.0, -1.0, 0.0]
    );

    let input: Vec<f32> = (0..24_000)
        .flat_map(|n| {
            let value = if (n / 24) % 2 == 0 {
                f32::MAX
            } else {
                f32::MIN
            };
            [value, value]
        })
        .collect();
    let mut capture = buffer(48_000, 2, 16_000, 20);
    let recording = record(&mut capture, &input, 2, || 512, true);
    assert_in_range(&recording.all_samples(), "48000 Hz x2 -> 16000 Hz");
}

#[test]
fn samples_beyond_full_scale_are_clipped_before_mixing() {
    // Clipping each channel first: (1 + 0) / 2, not (3 + 0) / 2 clipped to 1.
    let mut capture = buffer(16_000, 2, 16_000, 20);
    capture.push(&[3.0, 0.0, -5.0, 0.5, 1.5, -1.5]).unwrap();
    assert_eq!(capture.finish().unwrap().samples, [0.5, -0.25, 0.0]);
}

#[test]
fn a_zero_chunk_duration_is_rejected() {
    assert_rejected(48_000, 2, 16_000, 0);
    assert_rejected(16_000, 1, 16_000, 0);
}

#[test]
fn zero_channels_are_rejected() {
    assert_rejected(48_000, 0, 16_000, 20);
    assert_rejected(16_000, 0, 16_000, 20);
}

#[test]
fn a_zero_sample_rate_is_rejected() {
    assert_rejected(0, 1, 16_000, 20);
    assert_rejected(16_000, 1, 0, 20);
    assert_rejected(0, 1, 0, 20);
}

#[test]
fn sample_rates_outside_the_configured_range_are_rejected() {
    let tuning = resample_tuning();
    let (low, high) = (tuning.min_sample_rate_hz, tuning.max_sample_rate_hz);
    for bad in [1, low - 1, high + 1, u32::MAX] {
        assert_rejected(bad, 1, 16_000, 20);
        assert_rejected(bad, 2, 16_000, 20);
        assert_rejected(16_000, 1, bad, 20);
        assert_rejected(bad, 1, bad, 20);
    }
}

#[test]
fn the_accepted_range_comes_from_the_resampling_values() {
    let mut narrow = resample_tuning();
    narrow.min_sample_rate_hz = 16_000;
    narrow.max_sample_rate_hz = 48_000;
    assert!(CaptureBuffer::new(48_000, 2, 16_000, 20, &narrow).is_ok());
    assert!(CaptureBuffer::new(16_000, 1, 48_000, 20, &narrow).is_ok());
    assert!(CaptureBuffer::new(8_000, 1, 16_000, 20, &narrow).is_err());
    assert!(CaptureBuffer::new(96_000, 1, 16_000, 20, &narrow).is_err());
    assert!(CaptureBuffer::new(16_000, 1, 8_000, 20, &narrow).is_err());
    assert!(CaptureBuffer::new(16_000, 1, 96_000, 20, &narrow).is_err());
    // The same rates are fine with the wider range the tests normally run with.
    assert!(build(8_000, 1, 96_000, 20).is_ok());
}

#[test]
fn sample_rates_at_the_ends_of_the_range_are_accepted() {
    let tuning = resample_tuning();
    let (low, high) = (tuning.min_sample_rate_hz, tuning.max_sample_rate_hz);
    assert!(build(low, 1, low, 20).is_ok());
    assert!(build(high, 2, high, 20).is_ok());
    assert!(build(low, 1, 16_000, 20).is_ok());
    assert!(build(high, 2, 16_000, 20).is_ok());
}

#[test]
fn every_supported_device_format_is_accepted() {
    for device_hz in capture::DEVICE_RATES {
        for channels in capture::DEVICE_CHANNELS {
            for target_hz in capture::DEVICE_RATES {
                for chunk_ms in [1, 20, u32::MAX] {
                    if let Err(err) = build(device_hz, channels, target_hz, chunk_ms) {
                        panic!(
                            "{device_hz} Hz x{channels} -> {target_hz} Hz, {chunk_ms} ms: {err:?}"
                        );
                    }
                }
            }
        }
    }
}
