mod support;

use audio::{ResampleTuning, StreamResampler, downmix_to_mono, resample_to_mono};
use dictation_types::{AudioChunk, ErrorKind};

fn tone(hz: f32, sample_rate: u32, channels: u16, millis: u32) -> AudioChunk {
    let frames = (sample_rate as u64 * millis as u64 / 1_000) as usize;
    let mut samples = Vec::with_capacity(frames * channels as usize);
    for n in 0..frames {
        let value = 0.5 * (2.0 * std::f32::consts::PI * hz * n as f32 / sample_rate as f32).sin();
        for _ in 0..channels {
            samples.push(value);
        }
    }
    AudioChunk::new(samples, sample_rate, channels)
}

/// Counts upward zero crossings, skipping the first and last tenth where the filter settles.
fn pitch_hz(samples: &[f32], sample_rate: u32) -> f32 {
    let (start, end) = (samples.len() / 10, samples.len() * 9 / 10);
    let middle = &samples[start..end];
    let crossings = middle
        .windows(2)
        .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
        .count();
    crossings as f32 * sample_rate as f32 / middle.len() as f32
}

fn peak(samples: &[f32]) -> f32 {
    let (start, end) = (samples.len() / 10, samples.len() * 9 / 10);
    samples[start..end]
        .iter()
        .fold(0.0f32, |top, s| top.max(s.abs()))
}

const FORMATS: [(u32, u16); 6] = [
    (8_000, 1),
    (22_050, 1),
    (44_100, 2),
    (48_000, 1),
    (48_000, 2),
    (96_000, 2),
];
const TARGETS: [u32; 4] = [8_000, 16_000, 24_000, 48_000];

#[test]
fn a_recording_keeps_its_duration_in_any_format() {
    for (rate, channels) in FORMATS {
        for target in TARGETS {
            let out = resample_to_mono(
                &tone(440.0, rate, channels, 1_000),
                target,
                &crate::support::resample_tuning(),
            )
            .unwrap();
            assert_eq!((out.sample_rate_hz, out.channels), (target, 1));
            let millis = out.duration_ms() as i64;
            assert!(
                (millis - 1_000).abs() <= 2,
                "{rate} Hz x{channels} -> {target} Hz: {millis} ms"
            );
        }
    }
}

#[test]
fn a_recording_keeps_its_pitch_and_loudness() {
    for (rate, channels) in FORMATS {
        for target in TARGETS {
            let out = resample_to_mono(
                &tone(440.0, rate, channels, 1_000),
                target,
                &crate::support::resample_tuning(),
            )
            .unwrap();
            let pitch = pitch_hz(&out.samples, target);
            assert!(
                (pitch - 440.0).abs() < 5.0,
                "{rate} Hz x{channels} -> {target} Hz: pitch {pitch}"
            );
            let loudness = peak(&out.samples);
            assert!(
                (loudness - 0.5).abs() < 0.03,
                "{rate} Hz x{channels} -> {target} Hz: peak {loudness}"
            );
        }
    }
}

#[test]
fn audio_already_in_the_target_format_is_passed_through_unchanged() {
    let input = tone(300.0, 16_000, 1, 250);
    let out = resample_to_mono(&input, 16_000, &crate::support::resample_tuning()).unwrap();
    assert_eq!(out.samples, input.samples);
}

fn close(actual: &[f32], expected: &[f32]) -> bool {
    actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected)
            .all(|(a, e)| (a - e).abs() < 1e-6)
}

#[test]
fn channels_are_averaged_not_summed() {
    // Left at +0.6 and right at -0.2 average to +0.2.
    let mono = downmix_to_mono(&[0.6, -0.2, 0.6, -0.2], 2).unwrap();
    assert!(close(&mono, &[0.2, 0.2]), "{mono:?}");
    // Opposite channels cancel out instead of clipping.
    assert!(close(&downmix_to_mono(&[1.0, -1.0], 2).unwrap(), &[0.0]));
    let three = downmix_to_mono(&[0.3, 0.3, 0.3, 0.9, 0.9, 0.9], 3).unwrap();
    assert!(close(&three, &[0.3, 0.9]), "{three:?}");
}

#[test]
fn broken_samples_count_as_silence() {
    let mono = downmix_to_mono(&[f32::NAN, 0.5, f32::INFINITY, f32::NEG_INFINITY], 1).unwrap();
    assert_eq!(mono, vec![0.0, 0.5, 0.0, 0.0]);
    assert!(close(
        &downmix_to_mono(&[f32::NAN, 0.4], 2).unwrap(),
        &[0.2]
    ));
}

#[test]
fn an_incomplete_last_frame_is_dropped() {
    assert_eq!(downmix_to_mono(&[0.2, 0.4, 0.9], 2).unwrap().len(), 1);
}

#[test]
fn nothing_in_gives_nothing_out() {
    let out = resample_to_mono(
        &AudioChunk::new(Vec::new(), 48_000, 2),
        16_000,
        &crate::support::resample_tuning(),
    )
    .unwrap();
    assert!(out.samples.is_empty());
    assert_eq!((out.sample_rate_hz, out.channels), (16_000, 1));
}

#[test]
fn unusable_formats_are_rejected() {
    let silence = vec![0.0; 100];
    for (rate, channels, target) in [
        (0, 1, 16_000),
        (48_000, 0, 16_000),
        (48_000, 1, 0),
        (100, 1, 16_000),
        (48_000, 1, 1_000_000),
    ] {
        let err = resample_to_mono(
            &AudioChunk::new(silence.clone(), rate, channels),
            target,
            &crate::support::resample_tuning(),
        )
        .expect_err("must be rejected");
        assert_eq!(
            err.kind,
            ErrorKind::Validation,
            "{rate} Hz x{channels} -> {target} Hz"
        );
    }
    assert!(StreamResampler::new(48_000, 0, 16_000, &crate::support::resample_tuning()).is_err());
    assert!(StreamResampler::new(0, 1, 16_000, &crate::support::resample_tuning()).is_err());
}

fn stream(input: &AudioChunk, target: u32, piece: usize) -> Vec<f32> {
    let mut resampler = StreamResampler::new(
        input.sample_rate_hz,
        input.channels,
        target,
        &crate::support::resample_tuning(),
    )
    .unwrap();
    let mut out = Vec::new();
    for part in input.samples.chunks(piece * input.channels as usize) {
        out.extend(resampler.push_samples(part).unwrap());
    }
    out.extend(resampler.flush().unwrap());
    out
}

#[test]
fn audio_arriving_in_pieces_comes_out_whole_whatever_the_piece_size() {
    for (rate, channels) in FORMATS {
        let input = tone(440.0, rate, channels, 1_000);
        for piece in [1, 160, 441, 1_024, 4_800, 100_000] {
            let out = stream(&input, 16_000, piece);
            let expected = 16_000i64;
            assert!(
                (out.len() as i64 - expected).abs() <= 16,
                "{rate} Hz x{channels} in pieces of {piece}: {} samples",
                out.len()
            );
            let pitch = pitch_hz(&out, 16_000);
            assert!(
                (pitch - 440.0).abs() < 5.0,
                "{rate} Hz x{channels} in pieces of {piece}: pitch {pitch}"
            );
        }
    }
}

#[test]
fn streaming_gives_the_same_sound_as_converting_the_whole_recording() {
    let input = tone(440.0, 48_000, 2, 1_000);
    let whole = resample_to_mono(&input, 16_000, &crate::support::resample_tuning())
        .unwrap()
        .samples;
    let pieces = stream(&input, 16_000, 480);
    assert!((whole.len() as i64 - pieces.len() as i64).abs() <= 16);
    // Compare away from the edges, where the two filters settle differently.
    let n = whole.len().min(pieces.len());
    let worst = (n / 10..n * 9 / 10)
        .map(|i| (whole[i] - pieces[i]).abs())
        .fold(0.0f32, f32::max);
    assert!(worst < 0.02, "largest difference {worst}");
}

#[test]
fn a_short_recording_that_never_fills_a_block_still_comes_out_on_flush() {
    let input = tone(440.0, 48_000, 1, 10);
    let mut resampler =
        StreamResampler::new(48_000, 1, 16_000, &crate::support::resample_tuning()).unwrap();
    let early = resampler.push_samples(&input.samples).unwrap();
    let late = resampler.flush().unwrap();
    assert_eq!(early.len() + late.len(), 160, "10 ms at 16 kHz");
}

#[test]
fn a_resampler_can_be_reused_for_the_next_recording() {
    let input = tone(440.0, 44_100, 1, 500);
    let mut resampler =
        StreamResampler::new(44_100, 1, 16_000, &crate::support::resample_tuning()).unwrap();
    let mut lengths = Vec::new();
    for _ in 0..3 {
        let mut out = resampler.push_samples(&input.samples).unwrap();
        out.extend(resampler.flush().unwrap());
        lengths.push(out.len());
    }
    assert!(
        lengths.iter().all(|len| (*len as i64 - 8_000).abs() <= 16),
        "{lengths:?}"
    );
    assert!(
        resampler.flush().unwrap().is_empty(),
        "nothing is left after a flush"
    );
}

#[test]
fn a_reset_discards_what_was_held_back() {
    let mut resampler =
        StreamResampler::new(48_000, 1, 16_000, &crate::support::resample_tuning()).unwrap();
    resampler
        .push_samples(&tone(440.0, 48_000, 1, 5).samples)
        .unwrap();
    resampler.reset();
    assert!(resampler.flush().unwrap().is_empty());
}

#[test]
fn a_stream_already_in_the_target_format_passes_straight_through() {
    let input = tone(300.0, 16_000, 1, 100);
    let mut resampler =
        StreamResampler::new(16_000, 1, 16_000, &crate::support::resample_tuning()).unwrap();
    assert_eq!(
        resampler.push_samples(&input.samples).unwrap(),
        input.samples
    );
    assert!(resampler.flush().unwrap().is_empty());
}

#[test]
fn a_block_of_more_than_one_second_at_the_highest_rate_is_refused() {
    let base = crate::support::resample_tuning();
    let too_big = ResampleTuning {
        block_frames: base.max_sample_rate_hz as usize + 1,
        ..base.clone()
    };
    let err = too_big
        .validate()
        .expect_err("a block longer than a second was accepted");
    assert_eq!(err.kind, ErrorKind::Validation, "{err:?}");
    assert!(StreamResampler::new(48_000, 1, 16_000, &too_big).is_err());
    assert!(
        resample_to_mono(&AudioChunk::new(vec![0.0; 10], 48_000, 1), 16_000, &too_big).is_err()
    );

    let one_second = ResampleTuning {
        block_frames: base.max_sample_rate_hz as usize,
        ..base
    };
    assert_eq!(one_second.validate(), Ok(()));
}

#[test]
fn the_largest_finite_samples_never_break_a_conversion() {
    for (rate, channels) in FORMATS {
        let samples: Vec<f32> = (0..rate as usize / 4 * usize::from(channels))
            .map(|n| if (n / 7) % 2 == 0 { f32::MAX } else { f32::MIN })
            .collect();
        let chunk = AudioChunk::new(samples, rate, channels);
        for target in TARGETS {
            let label = format!("{rate} Hz x{channels} -> {target} Hz");
            let whole = resample_to_mono(&chunk, target, &crate::support::resample_tuning())
                .unwrap_or_else(|err| panic!("{label}: {err:?}"));
            let streamed = stream(&chunk, target, 333);
            for output in [&whole.samples, &streamed] {
                assert!(!output.is_empty(), "{label}");
                assert!(
                    output
                        .iter()
                        .all(|s| s.is_finite() && (-1.0..=1.0).contains(s)),
                    "{label}: a sample left full scale or is not finite"
                );
            }
        }
    }
}
