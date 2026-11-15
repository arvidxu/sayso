#[path = "support/capture.rs"]
mod capture;
mod support;

use audio::DeviceInbox;
use capture::{capture_tuning, device_audio, inbox, rms, tone};
use dictation_types::{AppError, InputLevel};
use support::Noise;

/// 400 Hz tones fill it with whole periods.
const WINDOW_MS: u32 = 50;
const TONE_HZ: f64 = 400.0;

const FORMATS: [(u32, u16); 4] = [(16_000, 1), (48_000, 1), (48_000, 2), (44_100, 2)];

fn new_inbox(device_hz: u32, channels: u16) -> (DeviceInbox, InputLevel) {
    inbox(device_hz, channels, &capture_tuning(WINDOW_MS, 3_600_000))
}

fn tone_ms(device_hz: u32, channels: u16, amplitude: f64, ms: usize) -> Vec<f32> {
    let frames = device_hz as usize * ms / 1_000;
    device_audio(
        &tone(device_hz, frames, TONE_HZ, amplitude),
        device_hz,
        channels,
    )
}

fn push_10ms_blocks(
    inbox: &mut DeviceInbox,
    level: &InputLevel,
    samples: &[f32],
    device_hz: u32,
    channels: u16,
) {
    let block = device_hz as usize / 100 * usize::from(channels);
    for piece in samples.chunks(block) {
        inbox.push(piece.iter().copied());
        assert_level_valid(level.get());
    }
}

fn assert_level_valid(level: f32) {
    assert!(
        level.is_finite() && (0.0..=1.0).contains(&level),
        "the level {level} is outside 0..=1"
    );
}

fn assert_close(level: f32, expected: f64, label: &str) {
    assert!(
        (f64::from(level) - expected).abs() <= 0.02 * expected.max(1e-3),
        "{label}: the level reads {level}, expected {expected:.4}"
    );
}

#[test]
fn the_level_is_zero_before_any_audio() {
    for (device_hz, channels) in FORMATS {
        let level = InputLevel::new();
        // Left over from an earlier recording.
        level.set(0.7);
        let mut inbox = DeviceInbox::new(
            device_hz,
            channels,
            &capture_tuning(WINDOW_MS, 3_600_000),
            level.clone(),
        )
        .unwrap();
        assert_eq!(level.get(), 0.0, "{device_hz} Hz x{channels}");
        inbox.push(Vec::new());
        assert_eq!(level.get(), 0.0, "an empty block moved the level");
    }
}

#[test]
fn the_level_is_zero_for_silence() {
    for (device_hz, channels) in FORMATS {
        let (mut inbox, level) = new_inbox(device_hz, channels);
        let silence = vec![0.0f32; device_hz as usize * usize::from(channels)];
        push_10ms_blocks(&mut inbox, &level, &silence, device_hz, channels);
        assert_eq!(level.get(), 0.0, "{device_hz} Hz x{channels}");
    }
}

#[test]
fn the_level_of_a_steady_tone_is_its_loudness() {
    for (device_hz, channels) in FORMATS {
        for amplitude in [0.01, 0.1, 0.5, 0.9] {
            let label = format!("{device_hz} Hz x{channels}, amplitude {amplitude}");
            let samples = tone_ms(device_hz, channels, amplitude, 500);
            let (mut inbox, level) = new_inbox(device_hz, channels);
            push_10ms_blocks(&mut inbox, &level, &samples, device_hz, channels);
            let window = device_hz as usize * WINDOW_MS as usize / 1_000 * usize::from(channels);
            let expected = rms(&samples[samples.len() - window..]);
            assert_close(level.get(), expected, &label);
        }
    }
}

#[test]
fn the_level_of_a_steady_tone_does_not_depend_on_how_the_device_blocks_it() {
    // One second of a 480 Hz tone plus one frame, so that the last frame sits on a zero
    // crossing: a level taken from too little audio reads far too low there.
    let amplitude = 0.8f64;
    let expected = amplitude / std::f64::consts::SQRT_2;
    for (device_hz, channels) in [(48_000u32, 1u16), (48_000, 2)] {
        let frames = device_hz as usize + 1;
        let mono = tone(device_hz, frames, 480.0, amplitude);
        let samples: Vec<f32> = mono
            .iter()
            .flat_map(|value| std::iter::repeat_n(*value, usize::from(channels)))
            .collect();
        let mut readings = Vec::new();
        for block_frames in [1usize, 2, 7, 64, 480, 4_096, usize::MAX / 4] {
            let (mut inbox, level) = new_inbox(device_hz, channels);
            let block = block_frames.saturating_mul(usize::from(channels));
            for piece in samples.chunks(block) {
                inbox.push(piece.iter().copied());
            }
            let label = format!("{device_hz} Hz x{channels} in blocks of {block_frames} frames");
            assert_close(level.get(), expected, &label);
            readings.push((block_frames, level.get()));
        }
        let first = readings[0].1;
        for (block_frames, reading) in &readings {
            assert_eq!(
                *reading, first,
                "{device_hz} Hz x{channels}: blocks of {block_frames} frames read differently: \
                 {readings:?}"
            );
        }
    }
}

#[test]
fn loud_then_quiet_reads_quiet() {
    for (device_hz, channels) in FORMATS {
        let label = format!("{device_hz} Hz x{channels}");
        let (mut inbox, level) = new_inbox(device_hz, channels);
        let loud = tone_ms(device_hz, channels, 0.8, 500);
        push_10ms_blocks(&mut inbox, &level, &loud, device_hz, channels);
        let quiet = tone_ms(device_hz, channels, 0.02, 300);
        push_10ms_blocks(&mut inbox, &level, &quiet, device_hz, channels);
        let window = device_hz as usize * WINDOW_MS as usize / 1_000 * usize::from(channels);
        assert_close(level.get(), rms(&quiet[quiet.len() - window..]), &label);
    }
}

#[test]
fn quiet_then_loud_reads_loud() {
    for (device_hz, channels) in FORMATS {
        let (mut inbox, level) = new_inbox(device_hz, channels);
        let quiet = tone_ms(device_hz, channels, 0.02, 500);
        push_10ms_blocks(&mut inbox, &level, &quiet, device_hz, channels);
        let loud = tone_ms(device_hz, channels, 0.8, 300);
        push_10ms_blocks(&mut inbox, &level, &loud, device_hz, channels);
        let window = device_hz as usize * WINDOW_MS as usize / 1_000 * usize::from(channels);
        assert_close(
            level.get(),
            rms(&loud[loud.len() - window..]),
            &format!("{device_hz} Hz x{channels}"),
        );
    }
}

#[test]
fn loud_then_silence_reads_zero() {
    for (device_hz, channels) in FORMATS {
        let (mut inbox, level) = new_inbox(device_hz, channels);
        let loud = tone_ms(device_hz, channels, 0.8, 500);
        push_10ms_blocks(&mut inbox, &level, &loud, device_hz, channels);
        assert!(level.get() > 0.3);
        let silent = vec![0.0f32; device_hz as usize * 3 / 10 * usize::from(channels)];
        push_10ms_blocks(&mut inbox, &level, &silent, device_hz, channels);
        assert_eq!(
            level.get(),
            0.0,
            "{device_hz} Hz x{channels}: 300 ms into silence"
        );
    }
}

#[test]
fn one_window_of_quiet_after_loud_input_reads_quiet_wherever_the_loud_part_ended() {
    // The loud part ends partway through a window: once a whole window of quiet input has
    // arrived, the most recent stretch of the window's length is all quiet.
    for (device_hz, channels) in FORMATS {
        for loud_ms in [500usize, 510, 525, 537] {
            let label = format!("{device_hz} Hz x{channels}, {loud_ms} ms loud");
            let (mut inbox, level) = new_inbox(device_hz, channels);
            let loud = tone_ms(device_hz, channels, 0.8, loud_ms);
            inbox.push(loud);
            let quiet = tone_ms(device_hz, channels, 0.02, WINDOW_MS as usize);
            inbox.push(quiet.iter().copied());
            assert_close(level.get(), rms(&quiet), &label);
        }
    }
}

#[test]
fn the_level_stays_within_zero_and_one_for_samples_beyond_full_scale() {
    for (device_hz, channels) in FORMATS {
        for scale in [1.5f32, 4.0, 1.0e6, 1.0e30, f32::MAX] {
            let mut noise = Noise::new(11);
            let samples: Vec<f32> = (0..device_hz as usize / 2 * usize::from(channels))
                .map(|_| noise.sample() * scale)
                .collect();
            let (mut inbox, level) = new_inbox(device_hz, channels);
            push_10ms_blocks(&mut inbox, &level, &samples, device_hz, channels);
        }
        let (mut inbox, level) = new_inbox(device_hz, channels);
        inbox.push(vec![f32::MAX; device_hz as usize * usize::from(channels)]);
        assert_eq!(
            level.get(),
            1.0,
            "{device_hz} Hz x{channels}: full scale and beyond"
        );
        inbox.push(vec![f32::MIN; device_hz as usize * usize::from(channels)]);
        assert_eq!(
            level.get(),
            1.0,
            "{device_hz} Hz x{channels}: full scale and beyond"
        );
    }
}

#[test]
fn non_finite_samples_count_as_silence_for_the_level() {
    for (device_hz, channels) in FORMATS {
        let label = format!("{device_hz} Hz x{channels}");
        let mut noise = Noise::new(5);
        let samples: Vec<f32> = (0..device_hz as usize / 2 * usize::from(channels))
            .map(|n| match n % 7 {
                0 => f32::NAN,
                2 => f32::INFINITY,
                4 => f32::NEG_INFINITY,
                _ => noise.sample(),
            })
            .collect();
        let (mut inbox, level) = new_inbox(device_hz, channels);
        push_10ms_blocks(&mut inbox, &level, &samples, device_hz, channels);
        assert!(
            level.get() > 0.0,
            "{label}: noise between broken samples reads 0"
        );

        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let (mut inbox, level) = new_inbox(device_hz, channels);
            let block = vec![bad; device_hz as usize / 2 * usize::from(channels)];
            push_10ms_blocks(&mut inbox, &level, &block, device_hz, channels);
            assert_eq!(
                level.get(),
                0.0,
                "{label}: only {bad} samples count as silence"
            );
        }
    }
}

#[test]
fn every_clone_of_the_level_reads_the_same_value() {
    let (mut inbox, level) = new_inbox(48_000, 2);
    let clones = [level.clone(), level.clone().clone()];
    inbox.push(tone_ms(48_000, 2, 0.5, 200));
    assert!(level.get() > 0.0);
    for clone in &clones {
        assert_eq!(clone.get(), level.get());
    }
    let reader = clones[0].clone();
    let from_thread = std::thread::spawn(move || reader.get()).join().unwrap();
    assert_eq!(from_thread, level.get());
}

#[test]
fn the_level_reads_zero_after_an_interruption() {
    for (device_hz, channels) in FORMATS {
        let (mut inbox, level) = new_inbox(device_hz, channels);
        let loud = tone_ms(device_hz, channels, 0.8, 300);
        push_10ms_blocks(&mut inbox, &level, &loud, device_hz, channels);
        assert!(level.get() > 0.3);
        inbox.interrupt(AppError::audio("the device was unplugged"));
        assert_eq!(level.get(), 0.0, "{device_hz} Hz x{channels}");
        push_10ms_blocks(&mut inbox, &level, &loud, device_hz, channels);
        assert_eq!(
            level.get(),
            0.0,
            "audio after the interruption moved the level"
        );
    }
}

#[test]
fn taking_samples_does_not_change_the_level() {
    let (mut inbox, level) = new_inbox(48_000, 2);
    inbox.push(tone_ms(48_000, 2, 0.5, 200));
    let before = level.get();
    inbox.take_samples();
    assert_eq!(level.get(), before);
}
