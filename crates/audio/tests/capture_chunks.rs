#[path = "support/capture.rs"]
mod capture;
mod support;

use capture::{assert_full_chunk, buffer, device_audio, drain, record, tone};
use support::{Noise, ramp_value};

/// (device rate, channels, target rate, chunk duration in ms, samples one chunk must hold).
const CHUNK_CASES: [(u32, u16, u32, u32, usize); 9] = [
    (48_000, 2, 16_000, 20, 320),
    (48_000, 1, 16_000, 100, 1_600),
    (44_100, 2, 16_000, 30, 480),
    (16_000, 1, 16_000, 10, 160),
    (16_000, 2, 16_000, 32, 512),
    (8_000, 1, 16_000, 20, 320),
    (96_000, 2, 8_000, 25, 200),
    (48_000, 2, 44_100, 10, 441),
    (22_050, 1, 48_000, 5, 240),
];

fn ramp(frames: usize) -> Vec<f32> {
    (0..frames).map(ramp_value).collect()
}

fn noise(frames: usize, seed: u64) -> Vec<f32> {
    let mut source = Noise::new(seed);
    (0..frames).map(|_| source.sample() * 0.9).collect()
}

#[test]
fn every_chunk_taken_while_recording_is_mono_at_the_target_rate_and_exactly_one_chunk_long() {
    for (device_hz, channels, target_hz, chunk_ms, chunk_len) in CHUNK_CASES {
        let label = format!("{device_hz} Hz x{channels} -> {target_hz} Hz, {chunk_ms} ms");
        let frames = device_hz as usize + 123;
        let input = device_audio(&tone(device_hz, frames, 330.0, 0.5), device_hz, channels);
        let mut sizes = Noise::new(u64::from(device_hz));
        let mut capture = buffer(device_hz, channels, target_hz, chunk_ms);
        let recording = record(
            &mut capture,
            &input,
            channels,
            || 1 + sizes.below(3_000),
            true,
        );

        assert!(
            !recording.chunks.is_empty(),
            "{label}: no chunk was ready while recording a second of audio"
        );
        for chunk in &recording.chunks {
            assert_full_chunk(chunk, target_hz, chunk_len, &label);
        }
        assert_eq!(
            recording.rest.channels, 1,
            "{label}: the remainder is not mono"
        );
        assert_eq!(
            recording.rest.sample_rate_hz, target_hz,
            "{label}: the remainder is not at the target rate"
        );
    }
}

#[test]
fn chunks_come_out_once_and_in_the_order_the_audio_arrived() {
    let input = ramp(16_000 * 2 + 77);
    for channels in [1u16, 2] {
        let device: Vec<f32> = input
            .iter()
            .flat_map(|value| std::iter::repeat_n(*value, usize::from(channels)))
            .collect();
        let mut sizes = Noise::new(7);
        let mut capture = buffer(16_000, channels, 16_000, 20);
        let recording = record(
            &mut capture,
            &device,
            channels,
            || 1 + sizes.below(900),
            true,
        );

        assert_eq!(recording.chunks.len(), input.len() / 320);
        for (index, chunk) in recording.chunks.iter().enumerate() {
            assert_eq!(
                chunk.samples,
                input[index * 320..(index + 1) * 320],
                "chunk {index} of a {channels}-channel device is not the audio that arrived then"
            );
        }
        assert_eq!(
            recording.rest.samples,
            input[recording.chunks.len() * 320..]
        );
    }
}

#[test]
fn a_chunk_that_was_taken_is_not_handed_out_again() {
    let input = ramp(1_000);
    let mut capture = buffer(16_000, 1, 16_000, 20);
    capture.push(&input).unwrap();
    let first = capture.take_chunk().expect("320 samples are ready");
    let second = capture.take_chunk().expect("640 samples are ready");
    let third = capture.take_chunk().expect("960 samples are ready");
    assert_eq!(first.samples, input[..320]);
    assert_eq!(second.samples, input[320..640]);
    assert_eq!(third.samples, input[640..960]);
    assert_eq!(capture.take_chunk(), None);
    assert_eq!(capture.finish().unwrap().samples, input[960..]);
}

#[test]
fn a_caller_that_never_takes_chunks_gets_the_whole_recording_at_the_end() {
    let input = ramp(16_000 + 5);
    let mut capture = buffer(16_000, 1, 16_000, 20);
    for block in input.chunks(333) {
        capture.push(block).unwrap();
    }
    let whole = capture.finish().unwrap();
    assert_eq!(whole.samples, input);
    assert_eq!((whole.sample_rate_hz, whole.channels), (16_000, 1));
}

#[test]
fn a_caller_that_takes_every_chunk_gets_only_the_rest_at_the_end() {
    for (device_hz, channels, target_hz, chunk_ms, chunk_len) in CHUNK_CASES {
        let label = format!("{device_hz} Hz x{channels} -> {target_hz} Hz, {chunk_ms} ms");
        let frames = device_hz as usize * 3 / 4 + 11;
        let input = device_audio(&tone(device_hz, frames, 250.0, 0.5), device_hz, channels);

        let mut keeping = buffer(device_hz, channels, target_hz, chunk_ms);
        let whole = record(&mut keeping, &input, channels, || 777, false)
            .rest
            .samples;

        let mut taking = buffer(device_hz, channels, target_hz, chunk_ms);
        let recording = record(&mut taking, &input, channels, || 777, true);
        let taken = recording.chunks.len() * chunk_len;
        assert!(taken > 0, "{label}: no chunk was handed out");
        assert_eq!(
            recording.rest.samples,
            whole[taken..],
            "{label}: the end returned something other than the audio after the taken chunks"
        );
    }
}

#[test]
fn chunks_left_untaken_are_returned_at_the_end_after_the_taken_ones() {
    let input = ramp(2_000);
    let mut capture = buffer(16_000, 1, 16_000, 20);
    capture.push(&input).unwrap();
    let first = capture.take_chunk().unwrap();
    assert_eq!(first.samples, input[..320]);
    assert_eq!(capture.finish().unwrap().samples, input[320..]);
}

#[test]
fn taking_a_chunk_before_any_audio_arrived_gives_nothing() {
    for (device_hz, channels, target_hz, chunk_ms, _) in CHUNK_CASES {
        let mut capture = buffer(device_hz, channels, target_hz, chunk_ms);
        for _ in 0..3 {
            assert_eq!(capture.take_chunk(), None);
        }
    }
}

#[test]
fn taking_a_chunk_gives_nothing_until_a_whole_chunk_has_arrived() {
    let input = ramp(640);
    let mut capture = buffer(16_000, 1, 16_000, 20);
    capture.push(&input[..319]).unwrap();
    assert_eq!(capture.take_chunk(), None, "319 of 320 samples");
    capture.push(&input[319..320]).unwrap();
    let chunk = capture.take_chunk().expect("320 samples arrived");
    assert_eq!(chunk.samples, input[..320]);
    assert_eq!(capture.take_chunk(), None, "nothing since the chunk");
    capture.push(&input[320..639]).unwrap();
    assert_eq!(capture.take_chunk(), None, "319 samples since the chunk");
    capture.push(&input[639..]).unwrap();
    assert_eq!(capture.take_chunk().unwrap().samples, input[320..]);
}

#[test]
fn too_little_converted_audio_gives_nothing_rather_than_a_short_chunk() {
    // 10 ms of device audio can never fill a 20 ms chunk, whatever the conversion.
    for (device_hz, channels, target_hz) in [(48_000u32, 2u16, 16_000u32), (8_000, 1, 16_000)] {
        let frames = device_hz as usize / 100;
        let input = device_audio(&tone(device_hz, frames, 300.0, 0.5), device_hz, channels);
        let mut capture = buffer(device_hz, channels, target_hz, 20);
        capture.push(&input).unwrap();
        assert_eq!(capture.take_chunk(), None, "{device_hz} Hz x{channels}");
        let rest = capture.finish().unwrap();
        assert!(
            rest.samples.len().abs_diff(160) <= 3,
            "{device_hz} Hz x{channels}: 10 ms came out as {} samples at 16 kHz",
            rest.samples.len()
        );
    }
}

#[test]
fn a_chunk_longer_than_the_recording_is_never_handed_out() {
    let input = ramp(48_000);
    let mut capture = buffer(16_000, 1, 16_000, u32::MAX);
    for block in input.chunks(4_800) {
        capture.push(block).unwrap();
        assert_eq!(capture.take_chunk(), None);
    }
    assert_eq!(capture.finish().unwrap().samples, input);
}

#[test]
fn mono_audio_at_the_target_rate_passes_through_unchanged() {
    for rate in [8_000u32, 16_000, 22_050, 44_100, 48_000, 96_000] {
        let frames = rate as usize / 2 + 3;
        let mut input = noise(frames, u64::from(rate));
        // Values a conversion would round or clip.
        input[0] = 1.0;
        input[1] = -1.0;
        input[2] = f32::MIN_POSITIVE;
        input[3] = 1.0e-7;
        input[4] = -0.999_999_94;
        for take in [false, true] {
            let mut sizes = Noise::new(3);
            let mut capture = buffer(rate, 1, rate, 20);
            let recording = record(&mut capture, &input, 1, || 1 + sizes.below(2_000), take);
            let output = recording.all_samples();
            assert_eq!(
                output.len(),
                input.len(),
                "{rate} Hz, taking chunks: {take}"
            );
            assert!(
                output
                    .iter()
                    .zip(&input)
                    .all(|(a, b)| a.to_bits() == b.to_bits()),
                "{rate} Hz, taking chunks: {take}: a sample was altered on the way through"
            );
        }
    }
}

#[test]
fn audio_in_the_target_format_is_available_as_soon_as_it_is_pushed() {
    let input = ramp(960);
    let mut capture = buffer(16_000, 1, 16_000, 20);
    capture.push(&input).unwrap();
    let chunks = drain(&mut capture);
    assert_eq!(chunks.len(), 3, "960 samples hold three 20 ms chunks");
}

#[test]
fn audio_in_the_target_format_beyond_full_scale_is_only_clipped() {
    let input = [
        0.5f32, 1.5, -2.0, 1.0, -1.0, 0.25, 1.000_001, -1.000_001, 7.0e10, -0.75,
    ];
    let clipped = [0.5f32, 1.0, -1.0, 1.0, -1.0, 0.25, 1.0, -1.0, 1.0, -0.75];
    for take in [false, true] {
        let mut capture = buffer(16_000, 1, 16_000, 1);
        let recording = record(&mut capture, &input, 1, || 3, take);
        assert_eq!(recording.all_samples(), clipped, "taking chunks: {take}");
    }
}

#[test]
fn a_chunk_duration_that_is_not_a_whole_number_of_samples_is_rounded_down() {
    // 1 ms at 22 050 Hz is 22.05 samples; 3 ms at 44 100 Hz is 132.3 samples.
    for (rate, chunk_ms, chunk_len) in [(22_050u32, 1u32, 22usize), (44_100, 3, 132)] {
        let input = ramp(chunk_len * 10 + 5);
        let mut capture = buffer(rate, 1, rate, chunk_ms);
        let recording = record(&mut capture, &input, 1, || 7, true);
        assert_eq!(recording.chunks.len(), 10, "{rate} Hz, {chunk_ms} ms");
        for chunk in &recording.chunks {
            assert_full_chunk(chunk, rate, chunk_len, &format!("{rate} Hz, {chunk_ms} ms"));
        }
        assert_eq!(recording.rest.samples, input[chunk_len * 10..]);
    }
}

#[test]
fn a_chunk_duration_below_one_sample_still_gives_chunks_of_one_sample() {
    // 1 ms at 800 Hz is 0.8 samples.
    let input = ramp(25);
    let mut capture = buffer(800, 1, 800, 1);
    capture.push(&input).unwrap();
    let chunks = drain(&mut capture);
    assert_eq!(chunks.len(), 25, "every sample is a chunk of its own");
    for (index, chunk) in chunks.iter().enumerate() {
        assert_eq!(chunk.samples, [input[index]]);
        assert_eq!((chunk.sample_rate_hz, chunk.channels), (800, 1));
    }
    assert!(capture.finish().unwrap().samples.is_empty());
}

#[test]
fn a_chunk_duration_below_one_sample_never_gives_empty_chunks_when_converting() {
    let input = tone(8_000, 4_000, 100.0, 0.5);
    let mut capture = buffer(8_000, 1, 800, 1);
    let recording = record(&mut capture, &input, 1, || 160, true);
    assert!(!recording.chunks.is_empty());
    for chunk in &recording.chunks {
        assert_full_chunk(chunk, 800, 1, "8000 Hz -> 800 Hz, 1 ms");
    }
    assert!(recording.all_samples().len().abs_diff(400) <= 3);
}
