#[path = "support/capture.rs"]
mod capture;
mod support;

use audio::resample_to_mono;
use capture::{
    DEVICE_CHANNELS, DEVICE_RATES, Recording, amplitude_at, buffer, device_audio, expected_len,
    middle, mix, record, rms, tone,
};
use dictation_types::AudioChunk;
use support::Noise;

const TARGET_RATES: [u32; 6] = [8_000, 16_000, 22_050, 44_100, 48_000, 96_000];

/// How far the handed-out length may be from the exact duration, in samples.
const LEN_SLACK: f64 = 3.0;

/// Both sides run the same conversion on the same blocks, so only rounding is allowed.
const SAMPLE_SLACK: f32 = 1e-6;

const TONE_HZ: f64 = 440.0;
const TONE_AMPLITUDE: f64 = 0.5;

fn tone_input(device_hz: u32, channels: u16) -> (usize, Vec<f32>) {
    let frames = device_hz as usize / 2;
    let mono = tone(device_hz, frames, TONE_HZ, TONE_AMPLITUDE);
    (frames, device_audio(&mono, device_hz, channels))
}

/// Three tones, all below half of every tested rate, sampled at `rate_hz`. Because of that
/// the same call at another rate gives exactly what a perfect conversion would.
fn rich_mono(rate_hz: u32, frames: usize) -> Vec<f32> {
    mix(
        &mix(
            &tone(rate_hz, frames, 310.0, 0.3),
            &tone(rate_hz, frames, 1_000.0, 0.2),
        ),
        &tone(rate_hz, frames, 2_350.0, 0.1),
    )
}

fn rich_input(device_hz: u32, channels: u16) -> (usize, Vec<f32>) {
    let frames = device_hz as usize / 2 + 7;
    let mono = rich_mono(device_hz, frames);
    (frames, device_audio(&mono, device_hz, channels))
}

fn irregular_blocks(seed: u64) -> impl FnMut() -> usize {
    let mut noise = Noise::new(seed);
    move || match noise.below(10) {
        0 => 1,
        1 => 2 + noise.below(5),
        2 => 4_000 + noise.below(9_000),
        _ => 1 + noise.below(1_200),
    }
}

fn assert_duration_kept(
    recording: &Recording,
    frames: usize,
    device_hz: u32,
    target_hz: u32,
    label: &str,
) {
    let expected = expected_len(frames, device_hz, target_hz);
    let actual = recording.all_samples().len() as f64;
    assert!(
        (actual - expected).abs() <= LEN_SLACK,
        "{label}: {actual} samples were handed out for {frames} device frames, expected {expected:.1}"
    );
}

fn assert_tone_kept(samples: &[f32], target_hz: u32, label: &str) {
    let steady = middle(samples);
    let amplitude = amplitude_at(steady, target_hz, TONE_HZ);
    assert!(
        (amplitude - TONE_AMPLITUDE).abs() < 0.02 * TONE_AMPLITUDE,
        "{label}: the 440 Hz tone came out with amplitude {amplitude:.4}, expected {TONE_AMPLITUDE}"
    );
    let loudness = rms(steady);
    let expected = TONE_AMPLITUDE / std::f64::consts::SQRT_2;
    assert!(
        (loudness - expected).abs() < 0.02 * expected,
        "{label}: loudness {loudness:.4}, expected {expected:.4} (something other than the tone came out)"
    );
}

fn one_shot(device_samples: &[f32], device_hz: u32, channels: u16, target_hz: u32) -> Vec<f32> {
    let chunk = AudioChunk::new(device_samples.to_vec(), device_hz, channels);
    resample_to_mono(&chunk, target_hz, &support::resample_tuning())
        .unwrap()
        .samples
}

/// Lengths must match exactly and every sample must agree.
fn one_shot_mismatch(
    recording: &Recording,
    device_samples: &[f32],
    (device_hz, channels, target_hz): (u32, u16, u32),
    label: &str,
) -> Option<String> {
    let expected = one_shot(device_samples, device_hz, channels, target_hz);
    let actual = recording.all_samples();
    assert!(
        actual
            .iter()
            .all(|sample| sample.is_finite() && (-1.0..=1.0).contains(sample)),
        "{label}: non-finite or out-of-range samples were handed out"
    );
    if actual.len() != expected.len() {
        return Some(format!(
            "{label}: {} samples were handed out, the one-shot conversion gives {}",
            actual.len(),
            expected.len()
        ));
    }
    let (worst_at, worst) = actual
        .iter()
        .zip(&expected)
        .map(|(a, b)| (a - b).abs())
        .enumerate()
        .fold(
            (0, 0.0f32),
            |best, (n, d)| if d > best.1 { (n, d) } else { best },
        );
    (worst > SAMPLE_SLACK).then(|| {
        format!(
            "{label}: differs from the one-shot conversion by up to {worst:.2e} at sample \
             {worst_at} of {}",
            actual.len()
        )
    })
}

fn assert_no_mismatch(mismatches: &[String]) {
    assert!(
        mismatches.is_empty(),
        "{} cases differ from the one-shot conversion by more than {SAMPLE_SLACK}:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}

#[test]
fn a_tone_keeps_duration_pitch_and_loudness_for_every_device_format_and_target_rate() {
    for device_hz in DEVICE_RATES {
        for channels in DEVICE_CHANNELS {
            for target_hz in TARGET_RATES {
                let label = format!("{device_hz} Hz x{channels} -> {target_hz} Hz");
                let (frames, input) = tone_input(device_hz, channels);
                let mut capture = buffer(device_hz, channels, target_hz, 20);
                let blocks = irregular_blocks(u64::from(device_hz + target_hz));
                let recording = record(&mut capture, &input, channels, blocks, true);
                assert_duration_kept(&recording, frames, device_hz, target_hz, &label);
                assert_tone_kept(&recording.all_samples(), target_hz, &label);
            }
        }
    }
}

#[test]
fn chunks_plus_remainder_equal_the_one_shot_conversion_for_every_format_and_target_rate() {
    let mut mismatches = Vec::new();
    for device_hz in DEVICE_RATES {
        for channels in DEVICE_CHANNELS {
            for target_hz in TARGET_RATES {
                let label = format!("{device_hz} Hz x{channels} -> {target_hz} Hz");
                let (frames, input) = rich_input(device_hz, channels);
                let mut capture = buffer(device_hz, channels, target_hz, 30);
                let blocks = irregular_blocks(u64::from(device_hz) * 3 + u64::from(target_hz));
                let recording = record(&mut capture, &input, channels, blocks, true);
                assert_duration_kept(&recording, frames, device_hz, target_hz, &label);
                mismatches.extend(one_shot_mismatch(
                    &recording,
                    &input,
                    (device_hz, channels, target_hz),
                    &label,
                ));
            }
        }
    }
    assert_no_mismatch(&mismatches);
}

const BLOCK_FORMATS: [(u32, u16, u32); 6] = [
    (48_000, 2, 16_000),
    (44_100, 1, 16_000),
    (8_000, 1, 16_000),
    (22_050, 2, 48_000),
    (96_000, 2, 8_000),
    (16_000, 1, 16_000),
];

#[test]
fn the_size_of_the_blocks_a_device_delivers_changes_nothing() {
    let mut mismatches = Vec::new();
    for (device_hz, channels, target_hz) in BLOCK_FORMATS {
        let (frames, input) = rich_input(device_hz, channels);
        let fixed_sizes = [
            1usize,
            2,
            3,
            37,
            441,
            1_023,
            1_024,
            1_025,
            4_097,
            usize::MAX,
        ];
        for size in fixed_sizes {
            let label =
                format!("{device_hz} Hz x{channels} -> {target_hz} Hz in blocks of {size} frames");
            let mut capture = buffer(device_hz, channels, target_hz, 20);
            let recording = record(&mut capture, &input, channels, || size, true);
            assert_duration_kept(&recording, frames, device_hz, target_hz, &label);
            mismatches.extend(one_shot_mismatch(
                &recording,
                &input,
                (device_hz, channels, target_hz),
                &label,
            ));
        }
        for seed in 1..=4u64 {
            let label =
                format!("{device_hz} Hz x{channels} -> {target_hz} Hz in irregular blocks {seed}");
            let mut capture = buffer(device_hz, channels, target_hz, 20);
            let recording = record(&mut capture, &input, channels, irregular_blocks(seed), true);
            assert_duration_kept(&recording, frames, device_hz, target_hz, &label);
            mismatches.extend(one_shot_mismatch(
                &recording,
                &input,
                (device_hz, channels, target_hz),
                &label,
            ));
        }
    }
    assert_no_mismatch(&mismatches);
}

#[test]
fn a_tone_delivered_one_frame_at_a_time_keeps_pitch_and_loudness() {
    for (device_hz, channels, target_hz) in BLOCK_FORMATS {
        let label = format!("{device_hz} Hz x{channels} -> {target_hz} Hz, one frame per block");
        let (frames, input) = tone_input(device_hz, channels);
        let mut capture = buffer(device_hz, channels, target_hz, 20);
        let recording = record(&mut capture, &input, channels, || 1, true);
        assert_duration_kept(&recording, frames, device_hz, target_hz, &label);
        assert_tone_kept(&recording.all_samples(), target_hz, &label);
    }
}

#[test]
fn the_chunk_duration_changes_nothing_about_the_audio() {
    let mut mismatches = Vec::new();
    for (device_hz, channels, target_hz) in BLOCK_FORMATS {
        let (frames, input) = rich_input(device_hz, channels);
        for chunk_ms in [
            1u32,
            3,
            10,
            20,
            33,
            100,
            250,
            499,
            500,
            501,
            10_000,
            u32::MAX,
        ] {
            let label =
                format!("{device_hz} Hz x{channels} -> {target_hz} Hz, {chunk_ms} ms chunks");
            let mut capture = buffer(device_hz, channels, target_hz, chunk_ms);
            let blocks = irregular_blocks(u64::from(chunk_ms));
            let recording = record(&mut capture, &input, channels, blocks, true);
            assert_duration_kept(&recording, frames, device_hz, target_hz, &label);
            mismatches.extend(one_shot_mismatch(
                &recording,
                &input,
                (device_hz, channels, target_hz),
                &label,
            ));
        }
    }
    assert_no_mismatch(&mismatches);
}

#[test]
fn recordings_of_any_length_keep_their_duration() {
    for (device_hz, channels, target_hz) in BLOCK_FORMATS {
        for frames in [1usize, 2, 5, 100, 1_023, 1_024, 1_025, 2_048, 5_000, 33_333] {
            let label = format!("{frames} frames at {device_hz} Hz x{channels} -> {target_hz} Hz");
            let mono = tone(device_hz, frames, 200.0, 0.4);
            let input = device_audio(&mono, device_hz, channels);
            let mut capture = buffer(device_hz, channels, target_hz, 20);
            let recording = record(&mut capture, &input, channels, irregular_blocks(9), true);
            assert_duration_kept(&recording, frames, device_hz, target_hz, &label);
            assert!(
                recording
                    .all_samples()
                    .iter()
                    .all(|s| s.is_finite() && (-1.0..=1.0).contains(s)),
                "{label}: non-finite or out-of-range samples were handed out"
            );
        }
    }
}

#[test]
fn taking_chunks_or_not_gives_the_same_audio() {
    for (device_hz, channels, target_hz) in BLOCK_FORMATS {
        let label = format!("{device_hz} Hz x{channels} -> {target_hz} Hz");
        let (_, input) = rich_input(device_hz, channels);
        let mut taking = buffer(device_hz, channels, target_hz, 20);
        let taken = record(&mut taking, &input, channels, irregular_blocks(5), true);
        let mut keeping = buffer(device_hz, channels, target_hz, 20);
        let kept = record(&mut keeping, &input, channels, irregular_blocks(5), false);
        assert!(!taken.chunks.is_empty(), "{label}: no chunk was handed out");
        assert!(kept.chunks.is_empty());
        assert_eq!(taken.all_samples(), kept.rest.samples, "{label}");
    }
}
