use audio::CaptureTuning;
use dictation_types::ErrorKind;

fn tuning(stream_timeout_ms: u64, level_window_ms: u32, max_recording_ms: u64) -> CaptureTuning {
    CaptureTuning {
        stream_timeout_ms,
        device_switch_poll_ms: 1,
        reopen_interval_ms: 1,
        level_window_ms,
        max_recording_ms,
    }
}

fn assert_refused_naming(values: CaptureTuning, name: &str) {
    let err = values
        .validate()
        .expect_err(&format!("{values:?} was accepted"));
    assert_eq!(err.kind, ErrorKind::Validation, "{values:?}: {err:?}");
    assert!(
        err.message.contains(name),
        "{values:?}: the error does not name {name}: {err:?}"
    );
}

#[test]
fn a_zero_timeout_is_refused() {
    assert_refused_naming(tuning(0, 50, 60_000), "stream_timeout_ms");
}

#[test]
fn a_zero_switch_poll_interval_is_refused() {
    let values = CaptureTuning {
        device_switch_poll_ms: 0,
        ..tuning(2_000, 50, 60_000)
    };
    assert_refused_naming(values, "device_switch_poll_ms");
}

#[test]
fn a_switch_poll_interval_longer_than_the_wait_for_a_device_is_refused() {
    let values = CaptureTuning {
        device_switch_poll_ms: 2_001,
        ..tuning(2_000, 50, 60_000)
    };
    assert_refused_naming(values, "device_switch_poll_ms");
    let equal = CaptureTuning {
        device_switch_poll_ms: 2_000,
        ..tuning(2_000, 50, 60_000)
    };
    assert!(equal.validate().is_ok());
}

#[test]
fn a_zero_level_window_is_refused() {
    assert_refused_naming(tuning(2_000, 0, 60_000), "level_window_ms");
}

#[test]
fn a_zero_maximum_recording_length_is_refused() {
    assert_refused_naming(tuning(2_000, 50, 0), "max_recording_ms");
}

#[test]
fn all_values_zero_are_refused() {
    let err = tuning(0, 0, 0).validate().expect_err("all zero");
    assert_eq!(err.kind, ErrorKind::Validation);
}

#[test]
fn a_level_window_longer_than_the_longest_recording_is_refused() {
    assert_refused_naming(tuning(2_000, 1_001, 1_000), "level_window_ms");
    assert_refused_naming(tuning(2_000, u32::MAX, 1), "level_window_ms");
}

#[test]
fn a_level_window_as_long_as_the_longest_recording_is_accepted() {
    assert_eq!(tuning(2_000, 1_000, 1_000).validate(), Ok(()));
    assert_eq!(tuning(1, 1, 1).validate(), Ok(()));
}

#[test]
fn positive_values_within_the_longest_recording_are_accepted() {
    for timeout in [1, 500, 2_000, 600_000, u64::MAX] {
        for window in [1, 50, 300, 60_000] {
            for max in [60_000, 3_600_000, u64::MAX] {
                assert_eq!(
                    tuning(timeout, window, max).validate(),
                    Ok(()),
                    "{timeout} ms timeout, {window} ms window, {max} ms maximum"
                );
            }
        }
    }
}

#[test]
fn the_timeout_is_not_limited_by_the_longest_recording() {
    assert_eq!(tuning(u64::MAX, 50, 1_000).validate(), Ok(()));
}
