#[path = "support/capture.rs"]
mod capture;
mod support;

use audio::CaptureBuffer;
use capture::{buffer, device_audio, expected_len, tone};

const FORMATS: [(u32, u16, u32); 3] = [
    (16_000, 1, 16_000),
    (48_000, 2, 16_000),
    (44_100, 1, 16_000),
];

fn input(device_hz: u32, channels: u16) -> (usize, Vec<f32>) {
    let frames = device_hz as usize * 7 / 10;
    let mono = tone(device_hz, frames, 300.0, 0.5);
    (frames, device_audio(&mono, device_hz, channels))
}

fn push_in_blocks(capture: &mut CaptureBuffer, samples: &[f32], channels: u16) {
    for block in samples.chunks(480 * usize::from(channels)) {
        capture.push(block).unwrap();
    }
}

#[test]
fn finishing_hands_over_the_recording_and_closes_the_buffer() {
    for (device_hz, channels, target_hz) in FORMATS {
        let label = format!("{device_hz} Hz x{channels} -> {target_hz} Hz");
        let (frames, samples) = input(device_hz, channels);
        let mut capture = buffer(device_hz, channels, target_hz, 20);
        push_in_blocks(&mut capture, &samples, channels);
        let recording = capture.finish().unwrap();
        assert!(
            capture.push(&[]).is_err(),
            "{label}: still open after finishing"
        );
        let expected = expected_len(frames, device_hz, target_hz);
        assert!(
            (recording.samples.len() as f64 - expected).abs() <= 3.0,
            "{label}: finishing returned {} samples, expected {expected:.1}",
            recording.samples.len()
        );
        assert_eq!(
            (recording.sample_rate_hz, recording.channels),
            (target_hz, 1)
        );
    }
}

#[test]
fn finishing_a_second_time_is_an_error() {
    for (device_hz, channels, target_hz) in FORMATS {
        let (_, samples) = input(device_hz, channels);
        let mut capture = buffer(device_hz, channels, target_hz, 20);
        push_in_blocks(&mut capture, &samples, channels);
        capture.finish().unwrap();
        for _ in 0..3 {
            assert!(
                capture.finish().is_err(),
                "{device_hz} Hz x{channels} -> {target_hz} Hz: finished again"
            );
        }
    }
}

#[test]
fn finishing_after_discarding_is_an_error() {
    let mut capture = buffer(48_000, 2, 16_000, 20);
    capture.discard();
    assert!(capture.finish().is_err());
}

#[test]
fn pushing_after_finishing_is_an_error_and_hands_out_nothing() {
    for (device_hz, channels, target_hz) in FORMATS {
        let label = format!("{device_hz} Hz x{channels} -> {target_hz} Hz");
        let (_, samples) = input(device_hz, channels);
        let mut capture = buffer(device_hz, channels, target_hz, 20);
        push_in_blocks(&mut capture, &samples, channels);
        capture.finish().unwrap();
        assert!(
            capture.push(&samples).is_err(),
            "{label}: audio accepted after finishing"
        );
        assert!(
            capture.push(&[]).is_err(),
            "{label}: empty block accepted after finishing"
        );
        assert_eq!(
            capture.take_chunk(),
            None,
            "{label}: a chunk after finishing"
        );
    }
}

#[test]
fn pushing_after_discarding_is_an_error_and_hands_out_nothing() {
    for (device_hz, channels, target_hz) in FORMATS {
        let label = format!("{device_hz} Hz x{channels} -> {target_hz} Hz");
        let (_, samples) = input(device_hz, channels);
        let mut capture = buffer(device_hz, channels, target_hz, 20);
        push_in_blocks(&mut capture, &samples[..samples.len() / 2], channels);
        capture.discard();
        assert!(
            capture.push(&samples).is_err(),
            "{label}: audio accepted after discarding"
        );
        assert_eq!(capture.take_chunk(), None, "{label}");
    }
}

#[test]
fn chunks_ready_at_the_end_are_not_handed_out_again_by_take_chunk() {
    for (device_hz, channels, target_hz) in FORMATS {
        let (_, samples) = input(device_hz, channels);
        let mut capture = buffer(device_hz, channels, target_hz, 20);
        push_in_blocks(&mut capture, &samples, channels);
        // Several chunks are ready but untaken: finishing hands them over, so taking one
        // afterwards would hand the same audio out twice.
        assert!(!capture.finish().unwrap().samples.is_empty());
        assert_eq!(capture.take_chunk(), None);
    }
}

#[test]
fn finishing_a_recording_without_audio_returns_empty_audio() {
    for (device_hz, channels, target_hz) in FORMATS {
        let mut capture = buffer(device_hz, channels, target_hz, 20);
        let recording = capture.finish().unwrap();
        assert!(recording.samples.is_empty());
        assert_eq!(
            (recording.sample_rate_hz, recording.channels),
            (target_hz, 1)
        );
    }
}

#[test]
fn finishing_after_only_empty_blocks_returns_empty_audio() {
    for (device_hz, channels, target_hz) in FORMATS {
        let mut capture = buffer(device_hz, channels, target_hz, 20);
        for _ in 0..5 {
            capture.push(&[]).unwrap();
        }
        assert!(capture.finish().unwrap().samples.is_empty());
    }
}

#[test]
fn discarding_throws_away_everything_not_yet_taken_and_closes_the_buffer() {
    for (device_hz, channels, target_hz) in FORMATS {
        let label = format!("{device_hz} Hz x{channels} -> {target_hz} Hz");
        let (_, samples) = input(device_hz, channels);
        let mut capture = buffer(device_hz, channels, target_hz, 20);
        push_in_blocks(&mut capture, &samples, channels);
        assert!(
            capture.take_chunk().is_some(),
            "{label}: 700 ms hold a chunk"
        );
        assert!(
            capture.take_chunk().is_some(),
            "{label}: 700 ms hold more than one chunk"
        );

        capture.discard();
        assert!(
            capture.push(&[]).is_err(),
            "{label}: still open after discarding"
        );
        assert_eq!(
            capture.take_chunk(),
            None,
            "{label}: a chunk after discarding"
        );
    }
}

#[test]
fn discarding_twice_or_after_finishing_does_no_harm() {
    let (_, samples) = input(48_000, 2);
    let mut capture = buffer(48_000, 2, 16_000, 20);
    push_in_blocks(&mut capture, &samples, 2);
    capture.discard();
    capture.discard();
    assert!(capture.finish().is_err());
    assert_eq!(capture.take_chunk(), None);

    let mut capture = buffer(48_000, 2, 16_000, 20);
    push_in_blocks(&mut capture, &samples, 2);
    assert!(!capture.finish().unwrap().samples.is_empty());
    capture.discard();
    assert_eq!(capture.take_chunk(), None);
    assert!(capture.finish().is_err());
}
