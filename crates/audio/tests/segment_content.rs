mod support;

use audio::{SegmentationConfig, segment_speech};
use dictation_types::AudioChunk;
use support::{
    INPUT_FORMATS, Noise, assert_pieces_faithful, ms_to_frames, ramp, ramp_value, silence, span,
    spans_ms,
};

fn limits(target_segment_ms: u64, max_segment_ms: u64, gap_ms: u32) -> SegmentationConfig {
    SegmentationConfig {
        target_segment_ms,
        max_segment_ms,
        gap_ms,
        min_part_ms: (target_segment_ms / 2).max(1),
        quiet_window_ms: 10,
    }
}

/// Speech of mixed lengths with pauses of mixed lengths, in milliseconds, within 6 seconds.
const TALK: [(u64, u64); 9] = [
    (0, 180),
    (400, 1_250),
    (1_300, 1_320),
    (1_900, 2_700),
    (2_700, 2_950),
    (3_100, 3_101),
    (3_500, 5_200),
    (5_400, 5_650),
    (5_700, 6_000),
];

#[test]
fn every_frame_of_every_span_appears_exactly_once_in_order() {
    for (rate, channels) in INPUT_FORMATS {
        let chunk = ramp(rate, channels, ms_to_frames(6_000, rate));
        let spans = spans_ms(rate, &TALK);
        for config in [
            limits(500, 1_000, 0),
            limits(500, 1_000, 120),
            limits(1, 1, 0),
            limits(40, 45, 7),
            limits(2_000, 30_000, 300),
            limits(60_000, 60_000, 50),
            limits(1, u64::MAX, 0),
        ] {
            let pieces = segment_speech(&chunk, &spans, &config).unwrap();
            assert!(!pieces.is_empty());
            assert_pieces_faithful(&chunk, &spans, &config, &pieces);
        }
    }
}

#[test]
fn speech_survives_any_spans_and_any_limits() {
    let mut noise = Noise::new(77);
    for round in 0..200 {
        let (rate, channels) = INPUT_FORMATS[noise.below(INPUT_FORMATS.len())];
        let frames = 1 + noise.below(rate as usize);
        let chunk = ramp(rate, channels, frames);
        let mut spans = Vec::new();
        let mut cursor = 0;
        while cursor < frames {
            let start = cursor + noise.below(frames / 6 + 1);
            let end = (start + noise.below(frames / 3 + 1)).min(frames);
            if start >= frames {
                break;
            }
            spans.push(span(start, end, rate));
            cursor = end;
        }
        let target = 1 + noise.below(400) as u64;
        let config = limits(
            target,
            target + noise.below(600) as u64,
            noise.below(80) as u32,
        );
        let pieces = segment_speech(&chunk, &spans, &config).unwrap();
        let real: Vec<_> = spans.iter().filter(|s| s.frames() > 0).cloned().collect();
        assert_eq!(pieces.is_empty(), real.is_empty(), "round {round}");
        assert_pieces_faithful(&chunk, &real, &config, &pieces);
    }
}

#[test]
fn with_no_gap_a_piece_holds_exactly_the_audio_of_its_spans() {
    for (rate, channels) in INPUT_FORMATS {
        let chunk = ramp(rate, channels, ms_to_frames(6_000, rate));
        let spans = spans_ms(rate, &TALK);
        let pieces = segment_speech(&chunk, &spans, &limits(60_000, 60_000, 0)).unwrap();
        assert_eq!(pieces.len(), 1);
        let expected = audio::keep_speech(&chunk, &spans).unwrap();
        assert_eq!(pieces[0].audio, expected);
        assert_eq!(
            pieces[0].spans.len(),
            spans.iter().filter(|s| s.frames() > 0).count()
        );
    }
}

#[test]
fn the_gap_is_silence_between_joined_spans_and_never_at_the_ends() {
    for (rate, channels) in INPUT_FORMATS {
        for gap_ms in [1, 20, 333] {
            let chunk = ramp(rate, channels, ms_to_frames(2_000, rate));
            let placed = [(100, 400), (700, 900), (1_500, 1_950)];
            let spans = spans_ms(rate, &placed);
            let pieces = segment_speech(&chunk, &spans, &limits(60_000, 60_000, gap_ms)).unwrap();
            assert_eq!(pieces.len(), 1);

            let gap = ms_to_frames(u64::from(gap_ms), rate);
            let mut expected = Vec::new();
            for (n, s) in spans.iter().enumerate() {
                if n > 0 {
                    expected.extend(std::iter::repeat_n(0.0, gap));
                }
                expected.extend((s.start_frame..s.end_frame).map(ramp_value));
            }
            let samples = &pieces[0].audio.samples;
            assert_eq!(samples.len(), expected.len(), "{rate} Hz, {gap_ms} ms gap");
            assert!(samples == &expected, "{rate} Hz, {gap_ms} ms gap");
            assert_ne!(samples[0], 0.0);
            assert_ne!(samples[samples.len() - 1], 0.0);
            assert_eq!(
                pieces[0].duration_ms(),
                (samples.len() as u64 * 1_000) / u64::from(rate)
            );
        }
    }
}

#[test]
fn a_piece_made_of_one_span_has_no_gap_at_all() {
    for (rate, channels) in INPUT_FORMATS {
        let chunk = ramp(rate, channels, ms_to_frames(1_000, rate));
        let spans = spans_ms(rate, &[(200, 700)]);
        let pieces = segment_speech(&chunk, &spans, &limits(5_000, 5_000, 250)).unwrap();
        assert_eq!(pieces.len(), 1);
        let expected: Vec<f32> = (spans[0].start_frame..spans[0].end_frame)
            .map(ramp_value)
            .collect();
        assert_eq!(pieces[0].audio.samples, expected);
    }
}

#[test]
fn pieces_count_from_zero_and_point_into_the_original_recording() {
    for (rate, channels) in INPUT_FORMATS {
        let chunk = ramp(rate, channels, ms_to_frames(4_000, rate));
        // Each span reaches the target on its own, so each becomes a piece.
        let placed = [(250, 900), (1_100, 1_800), (2_000, 2_600), (3_333, 3_999)];
        let spans = spans_ms(rate, &placed);
        let pieces = segment_speech(&chunk, &spans, &limits(500, 1_000, 100)).unwrap();
        assert_eq!(pieces.len(), placed.len(), "{rate} Hz x{channels}");
        for (n, (piece, (start_ms, end_ms))) in pieces.iter().zip(placed).enumerate() {
            assert_eq!(piece.index, n);
            assert_eq!(
                (piece.start_frame, piece.end_frame),
                (spans[n].start_frame, spans[n].end_frame)
            );
            // Milliseconds are whole, so they may fall up to one short of the placed position.
            assert!(
                piece.start_ms().abs_diff(start_ms) <= 1 && piece.end_ms().abs_diff(end_ms) <= 1
            );
            assert_eq!(piece.spans, [spans[n].clone()]);
            assert_eq!(
                (piece.audio.sample_rate_hz, piece.audio.channels),
                (rate, 1)
            );
        }
        assert_pieces_faithful(&chunk, &spans, &limits(500, 1_000, 100), &pieces);
    }
}

#[test]
fn a_piece_of_several_spans_reaches_from_its_first_span_to_its_last() {
    let rate = 22_050;
    let chunk = ramp(rate, 2, ms_to_frames(3_000, rate));
    let spans = spans_ms(rate, &[(500, 700), (1_000, 1_200), (2_500, 2_800)]);
    let pieces = segment_speech(&chunk, &spans, &limits(10_000, 10_000, 30)).unwrap();
    assert_eq!(pieces.len(), 1);
    assert_eq!(pieces[0].start_frame, spans[0].start_frame);
    assert_eq!(pieces[0].end_frame, spans[2].end_frame);
    assert_eq!((pieces[0].start_ms(), pieces[0].end_ms()), (500, 2_800));
    assert_eq!(pieces[0].spans, spans);
}

#[test]
fn stereo_input_gives_mono_pieces_holding_the_average_of_the_channels() {
    let samples = vec![
        0.2, 0.4, 1.0, -1.0, 0.5, 0.5, -0.25, -0.75, 0.0, 0.8, 0.6, 0.6,
    ];
    let chunk = AudioChunk::new(samples, 48_000, 2);
    let spans = [span(0, 2, 48_000), span(3, 6, 48_000)];
    let pieces = segment_speech(&chunk, &spans, &limits(1_000, 1_000, 0)).unwrap();
    assert_eq!(pieces.len(), 1);
    assert_eq!(
        (pieces[0].audio.sample_rate_hz, pieces[0].audio.channels),
        (48_000, 1)
    );
    let expected = [0.3, 0.0, -0.5, 0.4, 0.6];
    assert_eq!(pieces[0].audio.samples.len(), expected.len());
    for (got, want) in pieces[0].audio.samples.iter().zip(expected) {
        assert!((got - want).abs() < 1e-6, "{:?}", pieces[0].audio.samples);
    }
}

#[test]
fn no_spans_give_no_pieces() {
    for (rate, channels) in INPUT_FORMATS {
        let chunk = ramp(rate, channels, 5_000);
        assert!(
            segment_speech(&chunk, &[], &limits(500, 1_000, 100))
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn an_empty_recording_gives_no_pieces() {
    for (rate, channels) in INPUT_FORMATS {
        let chunk = silence(rate, channels, 0);
        assert!(
            segment_speech(&chunk, &[], &limits(500, 1_000, 100))
                .unwrap()
                .is_empty()
        );
        let nothing = [span(0, 0, rate)];
        assert!(
            segment_speech(&chunk, &nothing, &limits(500, 1_000, 100))
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn zero_length_spans_are_ignored() {
    for (rate, channels) in INPUT_FORMATS {
        let chunk = ramp(rate, channels, 5_000);
        let only_empty = [
            span(0, 0, rate),
            span(100, 100, rate),
            span(5_000, 5_000, rate),
        ];
        let config = limits(500, 1_000, 100);
        assert!(
            segment_speech(&chunk, &only_empty, &config)
                .unwrap()
                .is_empty()
        );

        let mixed = [
            span(0, 0, rate),
            span(100, 300, rate),
            span(400, 400, rate),
            span(500, 900, rate),
            span(5_000, 5_000, rate),
        ];
        let real = [mixed[1].clone(), mixed[3].clone()];
        let pieces = segment_speech(&chunk, &mixed, &config).unwrap();
        assert_eq!(pieces.len(), 1);
        assert_eq!(pieces[0].spans, real);
        assert_pieces_faithful(&chunk, &real, &config, &pieces);
        assert_eq!(pieces, segment_speech(&chunk, &real, &config).unwrap());
    }
}

#[test]
fn invalid_spans_are_rejected() {
    let config = limits(500, 1_000, 100);
    for channels in [1, 2] {
        let chunk = ramp(16_000, channels, 8_000);
        let cases: [(&str, Vec<audio::SpeechSpan>); 7] = [
            ("another sample rate", vec![span(0, 100, 8_000)]),
            (
                "another sample rate later",
                vec![span(0, 100, 16_000), span(200, 300, 48_000)],
            ),
            (
                "out of order",
                vec![span(4_000, 5_000, 16_000), span(100, 200, 16_000)],
            ),
            (
                "overlapping",
                vec![span(100, 300, 16_000), span(299, 400, 16_000)],
            ),
            ("reversed", vec![span(300, 100, 16_000)]),
            ("past the end", vec![span(7_000, 8_001, 16_000)]),
            (
                "far outside",
                vec![span(usize::MAX - 5, usize::MAX, 16_000)],
            ),
        ];
        for (what, spans) in cases {
            assert!(
                segment_speech(&chunk, &spans, &config).is_err(),
                "{what} x{channels}"
            );
        }
    }
    let stereo = ramp(16_000, 2, 8_000);
    assert!(segment_speech(&stereo, &[span(0, 16_000, 16_000)], &config).is_err());
}

#[test]
fn a_recording_without_channels_is_rejected() {
    let chunk = AudioChunk::new(vec![0.1; 1_000], 16_000, 0);
    let config = limits(500, 1_000, 0);
    assert!(segment_speech(&chunk, &[], &config).is_err());
    assert!(segment_speech(&chunk, &[span(0, 10, 16_000)], &config).is_err());
}

#[test]
fn segmenting_the_same_input_twice_gives_the_same_pieces() {
    let chunk = ramp(44_100, 2, ms_to_frames(6_000, 44_100));
    let spans = spans_ms(44_100, &TALK);
    let config = limits(700, 900, 60);
    assert_eq!(
        segment_speech(&chunk, &spans, &config).unwrap(),
        segment_speech(&chunk, &spans, &config).unwrap()
    );
}
