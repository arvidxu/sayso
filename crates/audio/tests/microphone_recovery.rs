#[path = "support/fake_devices.rs"]
mod fake_devices;

use std::time::{Duration, Instant};

use cpal::ErrorKind as DeviceErrorKind;
use dictation_api::AudioCaptureSession;
use dictation_types::ErrorKind;

use fake_devices::*;

#[test]
fn a_chosen_microphone_whose_format_changed_is_opened_again_and_the_recording_goes_on() {
    let devices = FakeDevices::with(&[SPEAKER, HEADSET], Some(HEADSET));
    let mut session = start(&devices, &config(Some(SPEAKER), false));
    devices.deliver(&chunk_of(0.25));
    assert_eq!(read(&mut session), Some(chunk_of(0.25)));

    // Another program starts a call and the device now records at another rate.
    devices.plug_in(SPEAKER, 48_000, 2);
    devices.fail(DeviceErrorKind::StreamInvalidated);
    assert!(session.is_active(), "the recording counts as going on");
    assert_eq!(session.interruption(), None);
    assert_eq!(read(&mut session), None, "nothing new arrived yet");
    assert_eq!(
        devices.opened(),
        [SPEAKER, SPEAKER],
        "the same device again"
    );

    // A tenth of a second of two channels at the new rate.
    devices.deliver(&vec![0.5; 48_000 / 10 * 2]);
    let mut after = read(&mut session).expect("the new stream's audio arrives");
    while let Some(chunk) = read(&mut session) {
        after.extend(chunk);
    }
    assert!(session.is_active());
    assert_eq!(session.dropouts(), 1);
    after.extend(session.stop().unwrap().samples);
    // Everything is in the same recording, converted to the rate asked for: a tenth of a
    // second, mixed to one channel at the value both channels had.
    let expected = RATE_HZ as usize / 10;
    assert!(
        after.len().abs_diff(expected) <= 1,
        "{} samples after the change, expected {expected}",
        after.len()
    );
    let middle = &after[after.len() / 10..after.len() * 9 / 10];
    assert!(
        middle.iter().all(|s| (s - 0.5).abs() < 0.01),
        "the converted audio is not the delivered value: {middle:?}"
    );
}

#[test]
fn a_device_reported_unavailable_or_busy_is_opened_again_if_it_is_still_listed() {
    for error in [
        DeviceErrorKind::DeviceNotAvailable,
        DeviceErrorKind::DeviceBusy,
    ] {
        let devices = FakeDevices::with(&[SPEAKER], None);
        let mut session = start(&devices, &config(Some(SPEAKER), false));
        devices.fail(error);
        assert_eq!(read(&mut session), None);
        devices.deliver(&chunk_of(0.75));
        assert_eq!(read(&mut session), Some(chunk_of(0.75)));
        assert_eq!(devices.opened(), [SPEAKER, SPEAKER]);
        assert!(session.is_active());
        assert_eq!(session.dropouts(), 1);
    }
}

#[test]
fn the_system_default_whose_format_changed_is_opened_again_as_the_default_once_the_system_keeps_it()
{
    let devices = FakeDevices::with(&[SPEAKER, HEADSET], Some(HEADSET));
    let mut session = start(&devices, &config(None, false));
    let stopped = Instant::now();
    devices.fail(DeviceErrorKind::StreamInvalidated);
    // The system may still pick another default; only when it has not within the patience is
    // the same device opened again.
    while devices.opened().len() < 2 {
        assert_eq!(read(&mut session), None);
        assert!(session.is_active(), "{:?}", session.interruption());
        assert!(stopped.elapsed() < Duration::from_millis(PATIENCE_MS * 2));
    }
    assert!(stopped.elapsed() >= Duration::from_millis(PATIENCE_MS));
    assert_eq!(devices.opened(), [HEADSET, HEADSET]);
    assert!(devices.asked().iter().all(|asked| asked == "default"));
    devices.deliver(&chunk_of(0.5));
    assert_eq!(read(&mut session), Some(chunk_of(0.5)));
    assert!(session.is_active());
}

#[test]
fn a_chosen_microphone_that_is_really_gone_ends_the_recording_and_keeps_its_audio() {
    let devices = FakeDevices::with(&[SPEAKER, HEADSET], Some(HEADSET));
    let mut session = start(&devices, &config(Some(SPEAKER), false));
    devices.deliver(&chunk_of(0.25));
    devices.unplug(SPEAKER);
    let stopped = Instant::now();
    devices.fail(DeviceErrorKind::DeviceNotAvailable);

    assert_eq!(read(&mut session), Some(chunk_of(0.25)));
    // It may only be away for a moment, so it is waited for as long as the patience allows.
    while session.is_active() {
        assert_eq!(read(&mut session), None);
        assert!(stopped.elapsed() < Duration::from_millis(PATIENCE_MS * 2));
    }
    assert!(stopped.elapsed() >= Duration::from_millis(PATIENCE_MS));
    let reason = session.interruption().expect("why it ended");
    assert_eq!(reason.kind, ErrorKind::AudioDevice, "{reason}");
    assert!(reason.message.contains(SPEAKER), "{reason}");
    assert_eq!(
        devices.opened(),
        [SPEAKER],
        "the system default was not used"
    );
}

#[test]
fn a_chosen_microphone_that_is_gone_is_replaced_by_the_default_when_allowed() {
    let devices = FakeDevices::with(&[SPEAKER, HEADSET], Some(HEADSET));
    let mut session = start(&devices, &config(Some(SPEAKER), true));
    devices.unplug(SPEAKER);
    devices.fail(DeviceErrorKind::DeviceNotAvailable);
    assert_eq!(read(&mut session), None);
    assert_eq!(devices.opened(), [SPEAKER, HEADSET]);
    devices.deliver(&chunk_of(0.5));
    assert_eq!(read(&mut session), Some(chunk_of(0.5)));
    assert!(session.is_active());
    assert_eq!(session.dropouts(), 1);
}

#[test]
fn a_device_that_cannot_be_used_any_more_is_not_opened_again() {
    let devices = FakeDevices::with(&[SPEAKER], None);
    let mut session = start(&devices, &config(Some(SPEAKER), false));
    devices.fail(DeviceErrorKind::PermissionDenied);
    assert_eq!(read(&mut session), None);
    assert!(!session.is_active());
    assert!(session.interruption().is_some());
    assert_eq!(devices.opened(), [SPEAKER]);
}

#[test]
fn a_device_that_will_not_open_again_in_time_ends_the_recording_with_the_reason() {
    let devices = FakeDevices::with(&[SPEAKER], None);
    let mut session = start(&devices, &config(Some(SPEAKER), false));
    devices.fail_next_opens(usize::MAX);
    devices.fail(DeviceErrorKind::StreamInvalidated);
    // Each read waits at most this long; trying goes on across reads.
    while session.is_active() {
        assert_eq!(read(&mut session), None);
    }
    let reason = session.interruption().expect("why it ended");
    assert!(reason.message.contains("still being set up"), "{reason}");
    assert_eq!(devices.opened(), [SPEAKER]);
}

#[test]
fn a_device_that_takes_a_moment_to_open_again_is_waited_for() {
    let devices = FakeDevices::with(&[SPEAKER], None);
    let mut session = start(&devices, &config(Some(SPEAKER), false));
    devices.fail_next_opens(3);
    devices.fail(DeviceErrorKind::StreamInvalidated);
    assert_eq!(read(&mut session), None);
    assert_eq!(devices.opened(), [SPEAKER, SPEAKER]);
    assert!(session.is_active());
}

#[test]
fn a_glitch_is_counted_and_the_stream_goes_on() {
    let devices = FakeDevices::with(&[SPEAKER], None);
    let mut session = start(&devices, &config(Some(SPEAKER), false));
    devices.fail(DeviceErrorKind::Xrun);
    devices.deliver(&chunk_of(0.5));
    assert_eq!(read(&mut session), Some(chunk_of(0.5)));
    assert_eq!(devices.opened(), [SPEAKER]);
    assert_eq!(session.dropouts(), 1);
    assert!(session.is_active());
}
