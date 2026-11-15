#[path = "support/capture.rs"]
mod capture;
mod support;

use audio::resample_to_mono;
use capture::{assert_full_chunk, buffer, device_audio, drain, tone};
use dictation_types::AudioChunk;
use support::resample_tuning;

const TARGET_HZ: u32 = 16_000;
const CHUNK_MS: u32 = 20;
const CHUNK_FRAMES: usize = 320;

/// (first device rate, channels, second device rate, channels).
const SWITCHES: [(u32, u16, u32, u16); 5] = [
    (48_000, 1, 48_000, 1),
    (48_000, 2, 44_100, 1),
    (44_100, 1, 16_000, 1),
    (16_000, 1, 48_000, 2),
    (96_000, 2, 8_000, 1),
];

fn device_tone(rate: u32, channels: u16, ms: usize, hz: f64) -> Vec<f32> {
    let frames = rate as usize * ms / 1_000;
    device_audio(&tone(rate, frames, hz, 0.4), rate, channels)
}

fn converted(samples: &[f32], rate: u32, channels: u16) -> Vec<f32> {
    resample_to_mono(
        &AudioChunk::new(samples.to_vec(), rate, channels),
        TARGET_HZ,
        &resample_tuning(),
    )
    .unwrap()
    .samples
}

fn push_in_blocks(
    capture: &mut audio::CaptureBuffer,
    samples: &[f32],
    channels: u16,
    block_frames: usize,
    taken: &mut Vec<AudioChunk>,
) {
    for block in samples.chunks(block_frames * usize::from(channels)) {
        capture.push(block).unwrap();
        taken.extend(drain(capture));
    }
}

#[test]
fn audio_from_both_devices_comes_out_exactly_as_two_separate_conversions() {
    for (first_hz, first_ch, second_hz, second_ch) in SWITCHES {
        let label = format!("{first_hz} Hz x{first_ch} -> {second_hz} Hz x{second_ch}");
        let before = device_tone(first_hz, first_ch, 730, 330.0);
        let after = device_tone(second_hz, second_ch, 910, 520.0);

        let mut capture = buffer(first_hz, first_ch, TARGET_HZ, CHUNK_MS);
        let mut taken = Vec::new();
        push_in_blocks(&mut capture, &before, first_ch, 441, &mut taken);
        capture.switch_device(second_hz, second_ch).unwrap();
        push_in_blocks(&mut capture, &after, second_ch, 512, &mut taken);
        let rest = capture.finish().unwrap();

        let mut out: Vec<f32> = taken.iter().flat_map(|c| c.samples.clone()).collect();
        out.extend_from_slice(&rest.samples);
        let mut expected = converted(&before, first_hz, first_ch);
        expected.extend(converted(&after, second_hz, second_ch));
        assert_eq!(out.len(), expected.len(), "{label}: length");
        let worst = out
            .iter()
            .zip(&expected)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        assert!(worst <= 1e-6, "{label}: differs by {worst}");
    }
}

#[test]
fn chunks_keep_their_size_across_a_switch() {
    for (first_hz, first_ch, second_hz, second_ch) in SWITCHES {
        let label = format!("{first_hz} Hz x{first_ch} -> {second_hz} Hz x{second_ch}");
        let mut capture = buffer(first_hz, first_ch, TARGET_HZ, CHUNK_MS);
        let mut taken = Vec::new();
        push_in_blocks(
            &mut capture,
            &device_tone(first_hz, first_ch, 333, 300.0),
            first_ch,
            480,
            &mut taken,
        );
        capture.switch_device(second_hz, second_ch).unwrap();
        push_in_blocks(
            &mut capture,
            &device_tone(second_hz, second_ch, 444, 300.0),
            second_ch,
            256,
            &mut taken,
        );
        assert!(!taken.is_empty(), "{label}: no chunks");
        for chunk in &taken {
            assert_full_chunk(chunk, TARGET_HZ, CHUNK_FRAMES, &label);
        }
    }
}

#[test]
fn switching_several_times_loses_nothing() {
    let formats = [(48_000, 1), (16_000, 1), (44_100, 2), (48_000, 1)];
    let mut capture = buffer(formats[0].0, formats[0].1, TARGET_HZ, CHUNK_MS);
    let mut taken = Vec::new();
    let mut expected = Vec::new();
    for (position, (rate, channels)) in formats.into_iter().enumerate() {
        if position > 0 {
            capture.switch_device(rate, channels).unwrap();
        }
        let part = device_tone(rate, channels, 250 + 100 * position, 440.0);
        push_in_blocks(&mut capture, &part, channels, 300, &mut taken);
        expected.extend(converted(&part, rate, channels));
    }
    let mut out: Vec<f32> = taken.iter().flat_map(|c| c.samples.clone()).collect();
    out.extend(capture.finish().unwrap().samples);
    assert_eq!(out.len(), expected.len());
}

#[test]
fn a_switch_to_an_unusable_format_is_refused_and_the_old_device_goes_on() {
    let mut capture = buffer(48_000, 1, TARGET_HZ, CHUNK_MS);
    let before = device_tone(48_000, 1, 200, 300.0);
    capture.push(&before).unwrap();
    assert!(capture.switch_device(0, 1).is_err());
    assert!(capture.switch_device(48_000, 0).is_err());
    assert!(capture.switch_device(1_000_000, 1).is_err());

    let more = device_tone(48_000, 1, 300, 300.0);
    capture.push(&more).unwrap();
    let mut all = before.clone();
    all.extend_from_slice(&more);
    let mut out: Vec<f32> = drain(&mut capture)
        .iter()
        .flat_map(|c| c.samples.clone())
        .collect();
    out.extend(capture.finish().unwrap().samples);
    assert_eq!(out, converted(&all, 48_000, 1));
}

#[test]
fn a_finished_or_discarded_buffer_cannot_switch() {
    let mut finished = buffer(48_000, 1, TARGET_HZ, CHUNK_MS);
    finished.finish().unwrap();
    assert!(finished.switch_device(44_100, 1).is_err());

    let mut discarded = buffer(48_000, 1, TARGET_HZ, CHUNK_MS);
    discarded.discard();
    assert!(discarded.switch_device(44_100, 1).is_err());
}

#[test]
fn the_converted_length_counts_every_device() {
    let mut capture = buffer(48_000, 2, TARGET_HZ, CHUNK_MS);
    assert_eq!(capture.converted_ms(), 0);
    capture.push(&device_tone(48_000, 2, 1_000, 300.0)).unwrap();
    capture.switch_device(16_000, 1).unwrap();
    // Everything of the first device is converted once the switch happened.
    assert_eq!(capture.converted_ms(), 1_000);
    capture.push(&device_tone(16_000, 1, 500, 300.0)).unwrap();
    assert_eq!(capture.converted_ms(), 1_500);
    // Taking chunks does not change how much was converted.
    drain(&mut capture);
    assert_eq!(capture.converted_ms(), 1_500);
}
