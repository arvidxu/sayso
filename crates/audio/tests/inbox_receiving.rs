#[path = "support/capture.rs"]
mod capture;
mod support;

use audio::DeviceInbox;
use capture::{capture_tuning, inbox};
use dictation_types::{AppError, ErrorKind, InputLevel};
use support::{Noise, ramp_value};

/// Every sample differs.
fn ramp(len: usize) -> Vec<f32> {
    (0..len).map(ramp_value).collect()
}

fn roomy() -> audio::CaptureTuning {
    capture_tuning(50, 3_600_000)
}

#[test]
fn nothing_is_handed_over_before_audio_arrives() {
    for channels in [1u16, 2] {
        let (mut inbox, _) = inbox(16_000, channels, &roomy());
        assert!(!inbox.has_samples());
        assert!(inbox.take_samples().is_empty());
        inbox.push(Vec::new());
        assert!(!inbox.has_samples());
        assert!(inbox.take_samples().is_empty());
    }
}

#[test]
fn samples_are_handed_over_in_order_and_exactly_once() {
    for channels in [1u16, 2] {
        let input = ramp(48_000 * usize::from(channels));
        let (mut inbox, _) = inbox(48_000, channels, &roomy());
        let mut sizes = Noise::new(u64::from(channels));
        let mut handed_over = Vec::new();
        let mut position = 0;
        while position < input.len() {
            let end = (position + 1 + sizes.below(3_000)).min(input.len());
            inbox.push(input[position..end].iter().copied());
            position = end;
            if sizes.below(3) == 0 {
                handed_over.extend(inbox.take_samples());
            }
        }
        handed_over.extend(inbox.take_samples());
        assert_eq!(handed_over, input, "{channels} channels");
        assert!(inbox.take_samples().is_empty(), "handed over twice");
        assert!(!inbox.has_samples());
    }
}

#[test]
fn taking_twice_in_a_row_gives_nothing_the_second_time() {
    let (mut inbox, _) = inbox(16_000, 2, &roomy());
    inbox.push(ramp(64));
    assert!(inbox.has_samples());
    assert_eq!(inbox.take_samples(), ramp(64));
    assert!(!inbox.has_samples());
    assert!(inbox.take_samples().is_empty());
}

#[test]
fn half_a_stereo_frame_is_held_back_until_the_rest_arrives() {
    let (mut inbox, _) = inbox(16_000, 2, &roomy());
    inbox.push([0.1]);
    assert!(!inbox.has_samples(), "half a frame counts as waiting audio");
    assert!(
        inbox.take_samples().is_empty(),
        "half a frame was handed over"
    );

    inbox.push([0.2, 0.3]);
    assert!(inbox.has_samples());
    assert_eq!(inbox.take_samples(), [0.1, 0.2]);
    assert!(!inbox.has_samples());

    inbox.push([0.4, 0.5, 0.6, 0.7]);
    assert_eq!(inbox.take_samples(), [0.3, 0.4, 0.5, 0.6]);
    assert!(!inbox.has_samples());
    inbox.push([0.8]);
    assert_eq!(inbox.take_samples(), [0.7, 0.8]);
    assert!(inbox.take_samples().is_empty());
}

#[test]
fn whatever_the_block_sizes_only_whole_frames_are_ever_handed_over() {
    for channels in [2u16, 3] {
        let width = usize::from(channels);
        let input = ramp(9_999 * width);
        let (mut inbox, _) = inbox(16_000, channels, &roomy());
        let mut sizes = Noise::new(9);
        let mut handed_over = Vec::new();
        let mut position = 0;
        while position < input.len() {
            let end = (position + 1 + sizes.below(17)).min(input.len());
            inbox.push(input[position..end].iter().copied());
            position = end;
            let taken = inbox.take_samples();
            assert_eq!(
                taken.len() % width,
                0,
                "{channels} channels: part of a frame was handed over"
            );
            handed_over.extend(taken);
        }
        assert_eq!(handed_over, input, "{channels} channels");
    }
}

#[test]
fn samples_are_stored_clipped_and_non_finite_samples_as_silence() {
    let (mut inbox, _) = inbox(16_000, 1, &roomy());
    inbox.push([
        2.0,
        -3.0,
        f32::MAX,
        f32::MIN,
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        0.5,
    ]);
    assert_eq!(
        inbox.take_samples(),
        [1.0, -1.0, 1.0, -1.0, 0.0, 0.0, 0.0, 0.5]
    );
}

#[test]
fn a_glitch_is_counted_and_recording_goes_on() {
    for channels in [1u16, 2] {
        let input = ramp(4_000 * usize::from(channels));
        let (mut inbox, level) = inbox(16_000, channels, &roomy());
        assert_eq!(inbox.dropouts(), 0);
        let half = input.len() / 2;
        inbox.push(input[..half].iter().copied());
        for expected in 1..=3 {
            inbox.note_dropout();
            assert_eq!(inbox.dropouts(), expected);
            assert!(inbox.is_receiving(), "a glitch ended the recording");
            assert!(inbox.interruption().is_none());
        }
        inbox.push(input[half..].iter().copied());
        assert_eq!(
            inbox.take_samples(),
            input,
            "audio after a glitch was not kept"
        );
        assert!(level.get() > 0.0, "the level stopped after a glitch");
    }
}

#[test]
fn a_glitch_before_any_audio_does_not_end_the_recording() {
    let (mut inbox, _) = inbox(16_000, 1, &roomy());
    inbox.note_dropout();
    inbox.push(ramp(10));
    assert_eq!(inbox.take_samples(), ramp(10));
    assert_eq!(inbox.dropouts(), 1);
}

#[test]
fn many_glitches_do_not_overflow_the_count() {
    let (mut inbox, _) = inbox(16_000, 1, &roomy());
    for _ in 0..100_000 {
        inbox.note_dropout();
    }
    assert_eq!(inbox.dropouts(), 100_000);
    assert!(inbox.is_receiving());
}

#[test]
fn after_an_interruption_audio_is_ignored_and_what_came_before_is_kept() {
    for channels in [1u16, 2] {
        let input = ramp(2_000 * usize::from(channels));
        let (mut inbox, _) = inbox(16_000, channels, &roomy());
        assert!(inbox.is_receiving());
        let half = input.len() / 2;
        inbox.push(input[..half].iter().copied());
        inbox.interrupt(AppError::audio("the device was unplugged"));
        assert!(!inbox.is_receiving());
        inbox.push(input[half..].iter().copied());
        assert!(inbox.has_samples());
        assert_eq!(
            inbox.take_samples(),
            input[..half],
            "{channels} channels: the audio around the interruption is wrong"
        );
        assert!(inbox.take_samples().is_empty());
        let reason = inbox.interruption().expect("the inbox was interrupted");
        assert!(
            reason.message.contains("the device was unplugged"),
            "{reason:?}"
        );
    }
}

#[test]
fn audio_taken_before_an_interruption_is_not_handed_over_again() {
    let (mut inbox, _) = inbox(16_000, 1, &roomy());
    inbox.push(ramp(100));
    assert_eq!(inbox.take_samples(), ramp(100));
    inbox.interrupt(AppError::audio("lost"));
    assert!(inbox.take_samples().is_empty());
}

#[test]
fn only_the_first_interruption_is_kept() {
    let (mut inbox, _) = inbox(16_000, 2, &roomy());
    inbox.push(ramp(100));
    inbox.interrupt(AppError::audio("first problem"));
    inbox.interrupt(AppError::timeout("second problem"));
    inbox.interrupt(AppError::audio("third problem"));
    let reason = inbox.interruption().expect("interrupted");
    assert!(reason.message.contains("first problem"), "{reason:?}");
    assert!(!reason.message.contains("second problem"), "{reason:?}");
    assert_ne!(reason.kind, ErrorKind::Timeout, "{reason:?}");
    assert!(!inbox.is_receiving());
    assert_eq!(inbox.take_samples(), ramp(100));
}

#[test]
fn an_interruption_before_any_audio_keeps_nothing() {
    let (mut inbox, _) = inbox(16_000, 1, &roomy());
    inbox.interrupt(AppError::audio("no device"));
    inbox.push(ramp(100));
    assert!(!inbox.has_samples());
    assert!(inbox.take_samples().is_empty());
    assert!(inbox.interruption().is_some());
}

#[test]
fn a_glitch_after_an_interruption_changes_nothing_about_it() {
    let (mut inbox, _) = inbox(16_000, 1, &roomy());
    inbox.interrupt(AppError::audio("lost"));
    inbox.note_dropout();
    assert!(!inbox.is_receiving());
    assert!(inbox.interruption().unwrap().message.contains("lost"));
}

fn build(rate: u32, channels: u16, tuning: &audio::CaptureTuning) -> Result<DeviceInbox, AppError> {
    DeviceInbox::new(rate, channels, tuning, InputLevel::new())
}

#[test]
fn a_zero_sample_rate_is_refused() {
    for channels in [1u16, 2] {
        let err = build(0, channels, &roomy()).expect_err("zero rate");
        assert_eq!(err.kind, ErrorKind::Validation, "{err:?}");
    }
}

#[test]
fn zero_channels_are_refused() {
    for rate in [8_000u32, 48_000] {
        let err = build(rate, 0, &roomy()).expect_err("zero channels");
        assert_eq!(err.kind, ErrorKind::Validation, "{err:?}");
    }
}

#[test]
fn ordinary_device_formats_are_accepted() {
    for rate in [8_000u32, 16_000, 44_100, 48_000, 96_000] {
        for channels in [1u16, 2, 6] {
            assert!(
                build(rate, channels, &roomy()).is_ok(),
                "{rate} Hz x{channels}"
            );
        }
    }
}
