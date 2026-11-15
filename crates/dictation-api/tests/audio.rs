mod support;

use dictation_api::AudioSource;
use dictation_types::{AppError, AudioDevice, ErrorKind};
use support::{Devices, device};

fn default_id(devices: Vec<AudioDevice>) -> Option<String> {
    Devices(Ok(devices))
        .default_device()
        .unwrap()
        .map(|d| d.id.as_str().to_owned())
}

#[test]
fn the_default_microphone_is_the_one_marked_default() {
    assert_eq!(
        default_id(vec![
            device("first", false),
            device("second", false),
            device("third", true)
        ])
        .as_deref(),
        Some("third")
    );
    assert_eq!(
        default_id(vec![
            device("first", false),
            device("second", true),
            device("third", false)
        ])
        .as_deref(),
        Some("second")
    );
    assert_eq!(
        default_id(vec![device("only", true)]).as_deref(),
        Some("only")
    );
}

#[test]
fn there_is_no_default_microphone_when_the_system_marks_none() {
    assert_eq!(
        default_id(vec![device("first", false), device("second", false)]),
        None
    );
}

#[test]
fn there_is_no_default_microphone_without_devices() {
    assert_eq!(default_id(Vec::new()), None);
}

#[test]
fn the_default_microphone_lookup_fails_when_devices_cannot_be_listed() {
    let source = Devices(Err(AppError::audio("no audio service")));
    assert_eq!(
        source.default_device().unwrap_err().kind,
        ErrorKind::AudioDevice
    );
}
