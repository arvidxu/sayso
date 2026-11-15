use dictation_types::{AudioCaptureConfig, AudioChunk, AudioDevice, DeviceId, InputLevel};

#[test]
fn audio_length_follows_sample_count_rate_and_channels() {
    assert_eq!(
        AudioChunk::new(vec![0.0; 16_000], 16_000, 1).duration_ms(),
        1_000
    );
    assert_eq!(
        AudioChunk::new(vec![0.0; 16_000], 16_000, 2).duration_ms(),
        500
    );
    assert_eq!(
        AudioChunk::new(vec![0.0; 24_000], 48_000, 1).duration_ms(),
        500
    );
    assert_eq!(AudioChunk::new(Vec::new(), 16_000, 1).duration_ms(), 0);
}

#[test]
fn audio_in_an_unusable_format_has_no_length() {
    assert_eq!(AudioChunk::new(vec![0.0; 100], 0, 1).duration_ms(), 0);
    assert_eq!(AudioChunk::new(vec![0.0; 100], 16_000, 0).duration_ms(), 0);
}

#[test]
fn a_capture_request_names_no_device_unless_told() {
    let config = AudioCaptureConfig::new(44_100, 20);
    assert_eq!(config.device_id, None);
    assert_eq!(config.sample_rate_hz, 44_100);
    assert_eq!(config.frame_duration_ms, 20);
}

#[test]
fn an_input_level_starts_silent_and_is_shared_by_its_clones() {
    let level = InputLevel::new();
    assert_eq!(level.get(), 0.0);
    let reader = level.clone();
    level.set(0.25);
    assert_eq!(reader.get(), 0.25);
}

#[test]
fn an_input_level_stays_between_zero_and_one() {
    let level = InputLevel::new();
    for (given, kept) in [
        (1.5, 1.0),
        (-0.2, 0.0),
        (f32::NAN, 0.0),
        (f32::INFINITY, 1.0),
        (f32::NEG_INFINITY, 0.0),
        (1.0, 1.0),
        (0.0, 0.0),
    ] {
        level.set(given);
        assert_eq!(level.get(), kept, "set to {given}");
    }
}

#[test]
fn an_input_level_can_be_read_from_another_thread_while_it_changes() {
    let level = InputLevel::new();
    let writer = level.clone();
    let handle = std::thread::spawn(move || {
        for step in 0..=100u8 {
            writer.set(f32::from(step) / 100.0);
        }
    });
    for _ in 0..100 {
        let value = level.get();
        assert!((0.0..=1.0).contains(&value));
    }
    handle.join().unwrap();
    assert_eq!(level.get(), 1.0);
}

#[test]
fn a_new_device_is_not_marked_as_default() {
    let device = AudioDevice::new(DeviceId::new("mic-1"), "Micrófono", 48_000, 1);
    assert!(!device.is_default);
    assert!(!device.is_bluetooth);
    assert!(!device.is_built_in);
    assert_eq!(device.name, "Micrófono");
}
