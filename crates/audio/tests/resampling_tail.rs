mod support;

use std::f64::consts::TAU;

use audio::{ResampleTuning, StreamResampler, resample_to_mono};
use dictation_types::AudioChunk;

const TARGET_HZ: u32 = 16_000;
/// Rates devices really report that share almost no factor with the target.
const UNUSUAL_RATES: [u32; 4] = [8_001, 11_025, 44_056, 48_001];
const BLOCKS: [usize; 3] = [64, 160, 1_024];

fn tuning(block_frames: usize) -> ResampleTuning {
    ResampleTuning {
        block_frames,
        ..support::resample_tuning()
    }
}

/// Frequencies share no ratio, so output shifted in time cannot match by accident.
fn sound(seconds: f64) -> f64 {
    0.15 * (TAU * 300.0 * seconds).sin() + 0.1 * (TAU * 1_234.567 * seconds).sin()
}

fn recording(rate: u32, frames: usize) -> AudioChunk {
    let samples = (0..frames)
        .map(|n| sound(n as f64 / f64::from(rate)) as f32)
        .collect();
    AudioChunk::new(samples, rate, 1)
}

/// Leaves out the very start and end, where the converter fades in and out.
fn worst_difference(out: &[f32], lag: f64) -> f64 {
    let (start, end) = (out.len() / 10, out.len() - out.len() / 50);
    (start..end)
        .map(|m| (f64::from(out[m]) - sound((m as f64 + lag) / f64::from(TARGET_HZ))).abs())
        .fold(0.0, f64::max)
}

/// The converter may run ahead by up to half an input sample when the rates are not whole
/// multiples of each other; no other time difference is allowed.
fn assert_same_sound(out: &[f32], rate: u32, what: &str) {
    let allowed_lag = 0.5 * f64::from(TARGET_HZ) / f64::from(rate);
    // Lags in tenths of an output sample, over a few samples either way.
    let (lag, worst) = (-30..=30)
        .map(|tenths| f64::from(tenths) / 10.0)
        .map(|lag| (lag, worst_difference(out, lag)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap();
    assert!(
        worst < 0.01,
        "{what} at {rate} Hz: the output is not the sound (off by {worst:.4} at best)"
    );
    assert!(
        lag.abs() <= allowed_lag + 0.1,
        "{what} at {rate} Hz: the output is {lag:.1} samples out of time"
    );
}

fn expected_len(frames: usize, rate: u32) -> f64 {
    frames as f64 * f64::from(TARGET_HZ) / f64::from(rate)
}

fn assert_same_duration(got: usize, frames: usize, rate: u32, what: &str) {
    let expected = expected_len(frames, rate);
    assert!(
        (got as f64 - expected).abs() <= 1.0,
        "{what}: {got} samples out for {frames} at {rate} Hz, expected {expected:.1}"
    );
}

#[test]
fn a_whole_recording_at_an_unusual_rate_keeps_its_duration_to_the_sample() {
    for rate in UNUSUAL_RATES {
        for block in BLOCKS {
            // Not a whole number of blocks, and long enough for several bursts.
            let frames = rate as usize * 3 / 2 + 37;
            let out =
                resample_to_mono(&recording(rate, frames), TARGET_HZ, &tuning(block)).unwrap();
            assert_same_duration(out.samples.len(), frames, rate, &format!("block {block}"));
            assert_same_sound(&out.samples, rate, &format!("block {block}"));
        }
    }
}

#[test]
fn audio_arriving_in_pieces_at_an_unusual_rate_keeps_its_duration_to_the_sample() {
    for rate in UNUSUAL_RATES {
        for block in BLOCKS {
            let frames = rate as usize + 11;
            let input = recording(rate, frames);
            let mut stream = StreamResampler::new(rate, 1, TARGET_HZ, &tuning(block)).unwrap();
            let mut out = Vec::new();
            for piece in input.samples.chunks(441) {
                out.extend(stream.push_samples(piece).unwrap());
            }
            out.extend(stream.flush().unwrap());
            assert_same_duration(out.len(), frames, rate, &format!("pieces, block {block}"));
            assert_same_sound(&out, rate, &format!("pieces, block {block}"));
        }
    }
}

#[test]
fn a_recording_shorter_than_one_burst_still_comes_out_whole() {
    for rate in UNUSUAL_RATES {
        for block in BLOCKS {
            let frames = block / 2 + 1;
            let out =
                resample_to_mono(&recording(rate, frames), TARGET_HZ, &tuning(block)).unwrap();
            assert_same_duration(
                out.samples.len(),
                frames,
                rate,
                &format!("short, block {block}"),
            );
        }
    }
}
