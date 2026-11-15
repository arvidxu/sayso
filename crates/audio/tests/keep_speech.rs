mod support;

use audio::keep_speech;
use dictation_types::AudioChunk;
use support::{INPUT_FORMATS, ramp, ramp_value, silence, span};

fn ramp_values(range: std::ops::Range<usize>) -> Vec<f32> {
    range.map(ramp_value).collect()
}

#[test]
fn the_result_holds_exactly_the_frames_of_the_spans_in_order() {
    for (rate, channels) in INPUT_FORMATS {
        let chunk = ramp(rate, channels, 10_000);
        let spans = [
            span(0, 10, rate),
            span(250, 1_000, rate),
            span(1_000, 1_001, rate),
            span(9_000, 10_000, rate),
        ];
        let kept = keep_speech(&chunk, &spans).unwrap();

        let mut expected = ramp_values(0..10);
        expected.extend(ramp_values(250..1_000));
        expected.extend(ramp_values(1_000..1_001));
        expected.extend(ramp_values(9_000..10_000));
        assert_eq!(kept.samples, expected, "{rate} Hz x{channels}");
        assert_eq!((kept.sample_rate_hz, kept.channels), (rate, 1));
    }
}

#[test]
fn stereo_frames_are_kept_as_the_average_of_their_channels() {
    let samples = vec![0.2, 0.4, 1.0, -1.0, 0.5, 0.5, -0.25, -0.75, 0.0, 0.8];
    let chunk = AudioChunk::new(samples, 44_100, 2);
    let kept = keep_speech(&chunk, &[span(1, 2, 44_100), span(3, 5, 44_100)]).unwrap();
    assert_eq!((kept.sample_rate_hz, kept.channels), (44_100, 1));
    let expected = [0.0, -0.5, 0.4];
    assert_eq!(kept.samples.len(), expected.len());
    for (got, want) in kept.samples.iter().zip(expected) {
        assert!((got - want).abs() < 1e-6, "{:?}", kept.samples);
    }
}

#[test]
fn a_span_over_the_whole_recording_keeps_everything() {
    for (rate, channels) in INPUT_FORMATS {
        let chunk = ramp(rate, channels, 3_210);
        let kept = keep_speech(&chunk, &[span(0, 3_210, rate)]).unwrap();
        assert_eq!(kept.samples, ramp_values(0..3_210));
    }
}

#[test]
fn no_spans_give_an_empty_result_in_the_recording_format() {
    for (rate, channels) in INPUT_FORMATS {
        let kept = keep_speech(&ramp(rate, channels, 500), &[]).unwrap();
        assert!(kept.samples.is_empty());
        assert_eq!((kept.sample_rate_hz, kept.channels), (rate, 1));
    }
}

#[test]
fn an_empty_recording_without_spans_gives_an_empty_result() {
    let kept = keep_speech(&AudioChunk::new(Vec::new(), 16_000, 2), &[]).unwrap();
    assert!(kept.samples.is_empty());
    assert_eq!((kept.sample_rate_hz, kept.channels), (16_000, 1));
}

#[test]
fn spans_that_touch_are_kept_without_a_seam() {
    let chunk = ramp(16_000, 1, 1_000);
    let kept = keep_speech(&chunk, &[span(100, 200, 16_000), span(200, 300, 16_000)]).unwrap();
    assert_eq!(kept.samples, ramp_values(100..300));
}

#[test]
fn zero_length_spans_add_nothing() {
    let chunk = ramp(16_000, 1, 1_000);
    let spans = [
        span(0, 0, 16_000),
        span(10, 20, 16_000),
        span(500, 500, 16_000),
        span(1_000, 1_000, 16_000),
    ];
    let kept = keep_speech(&chunk, &spans).unwrap();
    assert_eq!(kept.samples, ramp_values(10..20));
}

#[test]
fn spans_at_another_sample_rate_are_rejected() {
    let chunk = ramp(16_000, 1, 1_000);
    for other in [8_000, 15_999, 16_001, 48_000, 0] {
        assert!(
            keep_speech(&chunk, &[span(0, 100, other)]).is_err(),
            "{other} Hz"
        );
        let mixed = [span(0, 100, 16_000), span(200, 300, other)];
        assert!(keep_speech(&chunk, &mixed).is_err(), "{other} Hz second");
    }
}

#[test]
fn spans_out_of_order_are_rejected() {
    let chunk = ramp(16_000, 1, 1_000);
    let spans = [span(500, 600, 16_000), span(100, 200, 16_000)];
    assert!(keep_speech(&chunk, &spans).is_err());
}

#[test]
fn overlapping_spans_are_rejected() {
    let chunk = ramp(16_000, 1, 1_000);
    for spans in [
        [span(100, 300, 16_000), span(299, 400, 16_000)],
        [span(100, 300, 16_000), span(100, 300, 16_000)],
        [span(100, 900, 16_000), span(200, 300, 16_000)],
    ] {
        assert!(keep_speech(&chunk, &spans).is_err(), "{spans:?}");
    }
}

#[test]
fn reversed_spans_are_rejected() {
    let chunk = ramp(16_000, 1, 1_000);
    assert!(keep_speech(&chunk, &[span(300, 100, 16_000)]).is_err());
    assert!(keep_speech(&chunk, &[span(1, 0, 16_000)]).is_err());
}

#[test]
fn spans_outside_the_recording_are_rejected() {
    for channels in [1, 2] {
        let chunk = ramp(16_000, channels, 1_000);
        for (start, end) in [
            (0, 1_001),
            (999, 1_001),
            (1_001, 1_002),
            (5_000, 6_000),
            (0, usize::MAX),
            (usize::MAX - 1, usize::MAX),
        ] {
            assert!(
                keep_speech(&chunk, &[span(start, end, 16_000)]).is_err(),
                "{start}..{end} x{channels}"
            );
        }
    }
    // A stereo recording has half as many frames as samples.
    let stereo = ramp(16_000, 2, 1_000);
    assert!(keep_speech(&stereo, &[span(0, 2_000, 16_000)]).is_err());
}

#[test]
fn any_span_of_an_empty_recording_is_outside_it() {
    let empty = silence(16_000, 1, 0);
    assert!(keep_speech(&empty, &[span(0, 1, 16_000)]).is_err());
}

#[test]
fn a_recording_without_channels_is_rejected() {
    let chunk = AudioChunk::new(vec![0.1; 100], 16_000, 0);
    assert!(keep_speech(&chunk, &[]).is_err());
    assert!(keep_speech(&chunk, &[span(0, 10, 16_000)]).is_err());
}

#[test]
fn a_rejected_request_does_not_depend_on_which_span_is_wrong() {
    let chunk = ramp(16_000, 1, 1_000);
    let good = span(0, 100, 16_000);
    let bad = span(900, 1_100, 16_000);
    assert!(keep_speech(&chunk, &[good.clone(), bad.clone()]).is_err());
    assert!(keep_speech(&chunk, &[bad, good]).is_err());
}
