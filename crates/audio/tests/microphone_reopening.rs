#[path = "support/fake_devices.rs"]
mod fake_devices;

use std::time::{Duration, Instant};

use cpal::ErrorKind as DeviceErrorKind;
use dictation_api::AudioCaptureSession;
use dictation_types::ErrorKind;

use fake_devices::*;

/// Long enough for anything a test waits for to have happened, with room for a slow machine.
fn bound() -> Duration {
    Duration::from_millis(PATIENCE_MS * 4)
}

#[test]
fn a_device_that_stops_right_after_every_reopen_is_not_opened_in_a_tight_loop() {
    let devices = FakeDevices::with(&[SPEAKER], None);
    let mut session = start(&devices, &config(Some(SPEAKER), false));
    devices.stop_on_open(DeviceErrorKind::StreamInvalidated);
    let stopped = Instant::now();
    devices.fail(DeviceErrorKind::StreamInvalidated);
    while session.is_active() {
        let asked = Instant::now();
        let wait = Duration::from_millis(READ_MS);
        assert_eq!(session.read_chunk(wait).unwrap(), None);
        if session.is_active() {
            assert!(
                asked.elapsed() >= wait,
                "a read returned after {:?} instead of waiting {wait:?}",
                asked.elapsed()
            );
        }
        assert!(stopped.elapsed() < bound(), "the recording never ended");
    }
    // One open at the start, then at most one per reopen interval within the patience.
    let most = 2 + (PATIENCE_MS / REOPEN_MS) as usize;
    let opened = devices.opened().len();
    assert!(opened >= 2, "the device was not opened again");
    assert!(
        opened <= most,
        "opened {opened} times, at most {most} expected"
    );
    let reason = session.interruption().expect("why it ended");
    assert!(reason.message.contains("kept stopping"), "{reason}");
}

#[test]
fn a_recording_following_the_default_moves_to_the_new_default_rather_than_the_stopped_device() {
    let devices = FakeDevices::with(&[SPEAKER, HEADSET], Some(HEADSET));
    let mut session = start(&devices, &config(None, false));
    devices.fail(DeviceErrorKind::DeviceNotAvailable);
    // The system has not picked another default yet: the stopped device is not opened again.
    assert_eq!(session.read_chunk(Duration::from_millis(20)).unwrap(), None);
    assert_eq!(devices.opened(), [HEADSET]);
    assert!(session.is_active());
    devices.set_default(Some(SPEAKER));
    assert_eq!(read(&mut session), None);
    assert_eq!(devices.opened(), [HEADSET, SPEAKER]);
    devices.deliver(&chunk_of(0.5));
    assert_eq!(read(&mut session), Some(chunk_of(0.5)));
    assert!(session.is_active());
}

#[test]
fn a_stream_that_never_delivers_ends_the_recording_with_the_reason() {
    let devices = FakeDevices::with(&[SPEAKER], None);
    let mut session = start(&devices, &config(Some(SPEAKER), false));
    let opened = Instant::now();
    while session.is_active() {
        assert_eq!(read(&mut session), None);
        assert!(opened.elapsed() < bound(), "the recording never ended");
    }
    assert!(opened.elapsed() >= Duration::from_millis(PATIENCE_MS));
    let reason = session.interruption().expect("why it ended");
    assert_eq!(reason.kind, ErrorKind::AudioDevice, "{reason}");
    assert!(reason.message.contains("delivered no audio"), "{reason}");
    // Sound that only comes once the recording has ended is not taken.
    devices.deliver(&chunk_of(0.5));
    assert_eq!(session.level().get(), 0.0);
    assert_eq!(read(&mut session), None);
    assert_eq!(session.stop().unwrap().samples, Vec::<f32>::new());

    // The same for a stream opened again after the first one delivered.
    let devices = FakeDevices::with(&[SPEAKER], None);
    let mut session = start(&devices, &config(Some(SPEAKER), false));
    devices.deliver(&chunk_of(0.25));
    assert_eq!(read(&mut session), Some(chunk_of(0.25)));
    devices.fail(DeviceErrorKind::StreamInvalidated);
    assert_eq!(read(&mut session), None);
    assert_eq!(devices.opened(), [SPEAKER, SPEAKER]);
    let reopened = Instant::now();
    while session.is_active() {
        assert_eq!(read(&mut session), None);
        assert!(reopened.elapsed() < bound(), "the recording never ended");
    }
    let reason = session.interruption().expect("why it ended");
    assert!(reason.message.contains("delivered no audio"), "{reason}");
    assert_eq!(session.stop().unwrap().samples, Vec::<f32>::new());
}

#[test]
fn a_stream_that_delivers_slowly_is_not_given_up() {
    let devices = FakeDevices::with(&[SPEAKER], None);
    let mut session = start(&devices, &config(Some(SPEAKER), false));
    devices.deliver(&chunk_of(0.25));
    assert_eq!(read(&mut session), Some(chunk_of(0.25)));
    // Once audio arrived, a pause longer than the patience does not end the recording.
    let pause = Instant::now();
    while pause.elapsed() < Duration::from_millis(PATIENCE_MS * 2) {
        assert_eq!(read(&mut session), None);
    }
    assert!(session.is_active(), "{:?}", session.interruption());
    devices.deliver(&chunk_of(0.5));
    assert_eq!(read(&mut session), Some(chunk_of(0.5)));
}

#[test]
fn a_device_that_allows_one_stream_at_a_time_is_opened_again_after_its_old_stream_closed() {
    let devices = FakeDevices::with(&[SPEAKER], None);
    devices.make_exclusive();
    let mut session = start(&devices, &config(Some(SPEAKER), false));
    devices.fail(DeviceErrorKind::StreamInvalidated);
    assert_eq!(read(&mut session), None);
    assert_eq!(devices.opened(), [SPEAKER, SPEAKER]);
    assert!(session.is_active(), "{:?}", session.interruption());
    devices.deliver(&chunk_of(0.75));
    assert_eq!(read(&mut session), Some(chunk_of(0.75)));
}

#[test]
fn a_device_missing_from_the_list_for_a_moment_is_waited_for() {
    let devices = FakeDevices::with(&[SPEAKER], None);
    let mut session = start(&devices, &config(Some(SPEAKER), false));
    devices.unplug(SPEAKER);
    devices.fail(DeviceErrorKind::DeviceNotAvailable);
    assert_eq!(session.read_chunk(Duration::from_millis(20)).unwrap(), None);
    assert!(session.is_active(), "{:?}", session.interruption());
    assert_eq!(devices.opened(), [SPEAKER]);
    // The device is set up again and listed once more.
    devices.plug_in(SPEAKER, RATE_HZ, 1);
    assert_eq!(read(&mut session), None);
    assert_eq!(devices.opened(), [SPEAKER, SPEAKER]);
    devices.deliver(&chunk_of(0.5));
    assert_eq!(read(&mut session), Some(chunk_of(0.5)));
    assert!(session.is_active());
}
