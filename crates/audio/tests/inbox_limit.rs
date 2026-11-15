#[path = "support/capture.rs"]
mod capture;
mod support;

use capture::{capture_tuning, inbox};
use dictation_types::{AppError, ErrorKind};
use support::{Noise, ramp_value};

type BlockSizes = (&'static str, Box<dyn FnMut() -> usize>);

fn ramp(len: usize) -> Vec<f32> {
    (0..len).map(ramp_value).collect()
}

/// (device rate, maximum length in ms, frames that length holds).
const LIMITS: [(u32, u64, usize); 4] = [
    (16_000, 100, 1_600),
    (48_000, 250, 12_000),
    (44_100, 30, 1_323),
    (22_050, 1, 22),
];

fn assert_timed_out(inbox: &audio::DeviceInbox, label: &str) {
    assert!(
        !inbox.is_receiving(),
        "{label}: still receiving past the limit"
    );
    let reason = inbox
        .interruption()
        .unwrap_or_else(|| panic!("{label}: not interrupted past the limit"));
    assert_eq!(reason.kind, ErrorKind::Timeout, "{label}: {reason:?}");
}

fn deliver(
    inbox: &mut audio::DeviceInbox,
    input: &[f32],
    mut next_block: impl FnMut() -> usize,
) -> Vec<f32> {
    let mut handed_over = Vec::new();
    let mut position = 0;
    let mut deliveries = 0;
    while position < input.len() {
        let end = (position + next_block().max(1)).min(input.len());
        inbox.push(input[position..end].iter().copied());
        position = end;
        deliveries += 1;
        if deliveries % 3 == 0 {
            handed_over.extend(inbox.take_samples());
        }
    }
    handed_over.extend(inbox.take_samples());
    handed_over
}

#[test]
fn exactly_the_maximum_length_is_kept_whatever_the_block_sizes() {
    for (device_hz, max_ms, max_frames) in LIMITS {
        for channels in [1u16, 2] {
            let width = usize::from(channels);
            let input = ramp((max_frames * 3 + 17) * width);
            let block_sizes: [BlockSizes; 5] = [
                ("one sample", Box::new(|| 1)),
                ("odd sizes", Box::new(|| 7)),
                ("the whole input", Box::new(|| usize::MAX)),
                ("crossing the limit mid-block", {
                    let first = max_frames * width - 3;
                    let mut sent = false;
                    Box::new(move || {
                        if std::mem::replace(&mut sent, true) {
                            10
                        } else {
                            first
                        }
                    })
                }),
                ("irregular", {
                    let mut noise = Noise::new(u64::from(device_hz));
                    Box::new(move || 1 + noise.below(999))
                }),
            ];
            for (name, sizes) in block_sizes {
                let label = format!("{device_hz} Hz x{channels}, {max_ms} ms limit, {name}");
                let tuning = capture_tuning(1, max_ms);
                let (mut inbox, _) = inbox(device_hz, channels, &tuning);
                let handed_over = deliver(&mut inbox, &input, sizes);
                assert_eq!(
                    handed_over.len(),
                    max_frames * width,
                    "{label}: kept {} samples",
                    handed_over.len()
                );
                assert_eq!(handed_over, input[..max_frames * width], "{label}");
                assert_timed_out(&inbox, &label);
            }
        }
    }
}

#[test]
fn audio_after_the_limit_is_ignored() {
    let (mut inbox, level) = inbox(16_000, 2, &capture_tuning(1, 100));
    inbox.push(ramp(1_600 * 2 + 2));
    assert_eq!(inbox.take_samples().len(), 3_200);
    assert_eq!(level.get(), 0.0, "the level still moves after the limit");
    inbox.push(ramp(500));
    assert!(!inbox.has_samples());
    assert!(inbox.take_samples().is_empty());
    assert_timed_out(&inbox, "after more audio");
}

#[test]
fn the_recording_ends_as_soon_as_the_maximum_length_has_arrived() {
    for channels in [1u16, 2] {
        let label = format!("{channels} channels");
        let (mut inbox, _) = inbox(16_000, channels, &capture_tuning(1, 100));
        inbox.push(ramp(1_600 * usize::from(channels)));
        assert_timed_out(&inbox, &label);
        assert_eq!(inbox.take_samples().len(), 1_600 * usize::from(channels));
    }
}

#[test]
fn just_below_the_limit_the_recording_goes_on() {
    for channels in [1u16, 2] {
        let width = usize::from(channels);
        let (mut inbox, _) = inbox(16_000, channels, &capture_tuning(1, 100));
        inbox.push(ramp(1_599 * width));
        assert!(inbox.is_receiving());
        assert!(inbox.interruption().is_none());
        assert_eq!(inbox.take_samples().len(), 1_599 * width);
    }
}

#[test]
fn taking_samples_does_not_reset_the_limit() {
    let (mut inbox, _) = inbox(16_000, 1, &capture_tuning(1, 100));
    let mut handed_over = 0;
    for _ in 0..10 {
        inbox.push(ramp(400));
        handed_over += inbox.take_samples().len();
    }
    assert_eq!(handed_over, 1_600);
    assert_timed_out(&inbox, "taking samples between blocks");
}

#[test]
fn broken_samples_count_towards_the_limit() {
    let (mut inbox, _) = inbox(16_000, 1, &capture_tuning(1, 100));
    inbox.push(vec![f32::NAN; 1_000]);
    inbox.push(vec![f32::INFINITY; 1_000]);
    assert_eq!(inbox.take_samples(), vec![0.0; 1_600]);
    assert_timed_out(&inbox, "broken samples");
}

#[test]
fn an_earlier_interruption_is_not_replaced_by_the_timeout() {
    let (mut inbox, _) = inbox(16_000, 1, &capture_tuning(1, 100));
    inbox.push(ramp(800));
    inbox.interrupt(AppError::audio("the device was unplugged"));
    inbox.push(ramp(5_000));
    let reason = inbox.interruption().unwrap();
    assert_ne!(reason.kind, ErrorKind::Timeout, "{reason:?}");
    assert_eq!(inbox.take_samples(), ramp(800));
}
