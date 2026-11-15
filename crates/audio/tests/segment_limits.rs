mod support;

use audio::{AudioSegment, SegmentationConfig, segment_speech};
use dictation_types::AudioChunk;
use support::{
    INPUT_FORMATS, assert_pieces_faithful, interleave, ms_to_frames, ramp, span, spans_ms, square,
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

/// Eight stretches of 300 ms of speech, 200 ms apart.
fn even_talk(rate: u32) -> Vec<audio::SpeechSpan> {
    let placed: Vec<(u64, u64)> = (0..8).map(|n| (100 + n * 500, 400 + n * 500)).collect();
    spans_ms(rate, &placed)
}

fn spans_per_piece(pieces: &[AudioSegment]) -> Vec<usize> {
    pieces.iter().map(|piece| piece.spans.len()).collect()
}

fn segment_even_talk(rate: u32, channels: u16, config: &SegmentationConfig) -> Vec<AudioSegment> {
    let chunk = ramp(rate, channels, ms_to_frames(4_200, rate));
    let spans = even_talk(rate);
    let pieces = segment_speech(&chunk, &spans, config).unwrap();
    assert_pieces_faithful(&chunk, &spans, config, &pieces);
    pieces
}

#[test]
fn a_piece_is_closed_at_the_first_pause_after_it_reached_the_target() {
    for (rate, channels) in INPUT_FORMATS {
        // Two spans make 600 ms, which is past the target of 500 ms.
        let pieces = segment_even_talk(rate, channels, &limits(500, 5_000, 0));
        assert_eq!(
            spans_per_piece(&pieces),
            [2, 2, 2, 2],
            "{rate} Hz x{channels}"
        );
        // One span already reaches a target of 300 ms.
        let pieces = segment_even_talk(rate, channels, &limits(300, 5_000, 0));
        assert_eq!(spans_per_piece(&pieces), [1; 8], "{rate} Hz x{channels}");
        // Three spans are needed for 700 ms.
        let pieces = segment_even_talk(rate, channels, &limits(700, 5_000, 0));
        assert_eq!(spans_per_piece(&pieces), [3, 3, 2], "{rate} Hz x{channels}");
    }
}

#[test]
fn speech_stays_in_one_piece_while_the_target_is_not_reached() {
    for (rate, channels) in INPUT_FORMATS {
        for gap_ms in [0, 150] {
            let pieces = segment_even_talk(rate, channels, &limits(10_000, 10_000, gap_ms));
            assert_eq!(spans_per_piece(&pieces), [8], "{rate} Hz x{channels}");
        }
    }
}

#[test]
fn a_piece_is_closed_before_the_target_when_the_next_span_would_pass_the_maximum() {
    for (rate, channels) in INPUT_FORMATS {
        // Three spans make 900 ms; a fourth would make 1200 ms.
        let pieces = segment_even_talk(rate, channels, &limits(1_000, 1_000, 0));
        assert_eq!(spans_per_piece(&pieces), [3, 3, 2], "{rate} Hz x{channels}");
        // With 100 ms gaps three spans would make 1100 ms.
        let pieces = segment_even_talk(rate, channels, &limits(1_000, 1_000, 100));
        assert_eq!(
            spans_per_piece(&pieces),
            [2, 2, 2, 2],
            "{rate} Hz x{channels}"
        );
    }
}

#[test]
fn a_piece_may_be_exactly_as_long_as_the_maximum() {
    // Sample rates at which 300 ms and 50 ms are whole numbers of frames.
    for rate in [8_000, 16_000, 44_100, 48_000] {
        let pieces = segment_even_talk(rate, 1, &limits(1_000, 1_000, 50));
        assert_eq!(spans_per_piece(&pieces), [3, 3, 2], "{rate} Hz");
        assert_eq!(pieces[0].duration_ms(), 1_000);
    }
}

#[test]
fn a_gap_longer_than_the_maximum_keeps_every_span_apart() {
    for (rate, channels) in INPUT_FORMATS {
        for gap_ms in [1_001, 60_000, u32::MAX] {
            let pieces = segment_even_talk(rate, channels, &limits(1_000, 1_000, gap_ms));
            assert_eq!(spans_per_piece(&pieces), [1; 8], "{rate} Hz x{channels}");
        }
    }
}

#[test]
fn no_piece_is_longer_than_a_maximum_of_one_millisecond() {
    for (rate, channels) in INPUT_FORMATS {
        let chunk = ramp(rate, channels, ms_to_frames(300, rate));
        let spans = spans_ms(rate, &[(10, 110), (150, 151), (200, 300)]);
        for gap_ms in [0, 1, 5] {
            let config = limits(1, 1, gap_ms);
            let pieces = segment_speech(&chunk, &spans, &config).unwrap();
            assert_pieces_faithful(&chunk, &spans, &config, &pieces);
            assert!(
                pieces.len() >= 201,
                "{rate} Hz x{channels}: {} pieces",
                pieces.len()
            );
            assert!(pieces.iter().all(|piece| piece.duration_ms() <= 1));
        }
    }
}

#[test]
fn a_span_longer_than_the_maximum_is_cut_into_parts_within_the_maximum() {
    for (rate, channels) in INPUT_FORMATS {
        let chunk = ramp(rate, channels, ms_to_frames(10_000, rate));
        let spans = spans_ms(rate, &[(120, 9_870)]);
        for config in [
            limits(500, 1_000, 0),
            limits(1_000, 1_000, 0),
            limits(10, 3_000, 0),
            limits(999, 1_000, 200),
        ] {
            let pieces = segment_speech(&chunk, &spans, &config).unwrap();
            assert_pieces_faithful(&chunk, &spans, &config, &pieces);
            let fewest = 9_750u64.div_ceil(config.max_segment_ms) as usize;
            assert!(pieces.len() >= fewest, "{rate} Hz x{channels}: {config:?}");
        }
    }
}

#[test]
fn a_span_exactly_as_long_as_the_maximum_is_not_cut() {
    for (rate, channels) in INPUT_FORMATS {
        let chunk = ramp(rate, channels, ms_to_frames(3_000, rate));
        let spans = spans_ms(rate, &[(500, 2_500)]);
        let pieces = segment_speech(&chunk, &spans, &limits(700, 2_000, 0)).unwrap();
        assert_eq!(pieces.len(), 1, "{rate} Hz x{channels}");
        assert_eq!(pieces[0].spans, spans);
    }
}

#[test]
fn uninterrupted_speech_cut_into_parts_gets_no_silence_put_into_it() {
    // One span means nothing was removed from the speech, so there is nothing to mark with a
    // gap: the pieces together must be exactly as long as the span.
    for (rate, channels) in INPUT_FORMATS {
        let chunk = ramp(rate, channels, ms_to_frames(3_000, rate));
        let spans = spans_ms(rate, &[(0, 3_000)]);
        let pieces = segment_speech(&chunk, &spans, &limits(1_000, 1_500, 100)).unwrap();
        let total: usize = pieces.iter().map(|piece| piece.audio.samples.len()).sum();
        let silent: usize = pieces
            .iter()
            .map(|piece| piece.audio.samples.iter().filter(|s| **s == 0.0).count())
            .sum();
        assert_eq!(
            (total, silent),
            (spans[0].frames(), 0),
            "{rate} Hz x{channels}: silence was inserted in the middle of continuous speech"
        );
    }
}

/// Quieter stretches given as (start ms, end ms, amplitude).
fn loud_with_dips(rate: u32, channels: u16, total_ms: u64, dips: &[(u64, u64, f32)]) -> AudioChunk {
    let mut mono = square(ms_to_frames(total_ms, rate), 0.8);
    for (start_ms, end_ms, amplitude) in dips {
        for sample in &mut mono[ms_to_frames(*start_ms, rate)..ms_to_frames(*end_ms, rate)] {
            *sample *= amplitude / 0.8;
        }
    }
    interleave(&mono, rate, channels)
}

/// No gap is configured, so pieces hold the span's audio unchanged; cuts must fall within
/// 10 ms of the given stretches, in order.
fn assert_cut_in(
    chunk: &AudioChunk,
    span_ms: (u64, u64),
    config: &SegmentationConfig,
    cuts_ms: &[(u64, u64)],
) {
    let rate = chunk.sample_rate_hz;
    let what = format!("{rate} Hz x{}", chunk.channels);
    let spans = spans_ms(rate, &[span_ms]);
    let pieces = segment_speech(chunk, &spans, config).unwrap();

    let mono = audio::keep_speech(chunk, &spans).unwrap().samples;
    let joined: Vec<f32> = pieces
        .iter()
        .flat_map(|piece| piece.audio.samples.clone())
        .collect();
    assert!(
        joined == mono,
        "{what}: the pieces do not add up to the span"
    );
    for piece in &pieces {
        assert!(
            piece.duration_ms() <= config.max_segment_ms,
            "{what}: {} ms",
            piece.duration_ms()
        );
    }

    let slack = ms_to_frames(10, rate);
    assert_eq!(
        pieces.len(),
        cuts_ms.len() + 1,
        "{what}: cut at {:?}",
        ends_ms(&pieces)
    );
    for (piece, (from_ms, to_ms)) in pieces.iter().zip(cuts_ms) {
        let (from, to) = (ms_to_frames(*from_ms, rate), ms_to_frames(*to_ms, rate));
        assert!(
            piece.end_frame + slack >= from && piece.end_frame <= to + slack,
            "{what}: cut at {} ms, not in the quiet stretch {from_ms}..{to_ms} ms",
            piece.end_ms()
        );
    }
}

fn ends_ms(pieces: &[AudioSegment]) -> Vec<u64> {
    pieces.iter().map(AudioSegment::end_ms).collect()
}

#[test]
fn a_long_span_is_cut_in_its_one_quiet_stretch() {
    for (rate, channels) in INPUT_FORMATS {
        for dip_start in [520, 650, 800, 930, 965] {
            let chunk = loud_with_dips(rate, channels, 1_500, &[(dip_start, dip_start + 30, 0.0)]);
            assert_cut_in(
                &chunk,
                (0, 1_500),
                &limits(500, 1_000, 0),
                &[(dip_start, dip_start + 30)],
            );
        }
    }
}

#[test]
fn a_long_span_is_cut_where_it_is_quietest_not_merely_quieter() {
    for (rate, channels) in INPUT_FORMATS {
        // A slightly quieter stretch first, a nearly silent one later.
        let chunk = loud_with_dips(rate, channels, 1_500, &[(600, 630, 0.3), (850, 880, 0.02)]);
        assert_cut_in(&chunk, (0, 1_500), &limits(500, 1_000, 0), &[(850, 880)]);
        // And the other way round.
        let chunk = loud_with_dips(rate, channels, 1_500, &[(600, 630, 0.02), (850, 880, 0.3)]);
        assert_cut_in(&chunk, (0, 1_500), &limits(500, 1_000, 0), &[(600, 630)]);
    }
}

#[test]
fn the_cut_is_found_relative_to_where_the_span_starts() {
    for (rate, channels) in INPUT_FORMATS {
        // The span starts 700 ms into the recording; an earlier quiet stretch is outside it.
        let chunk = loud_with_dips(
            rate,
            channels,
            2_400,
            &[(300, 330, 0.0), (1_500, 1_530, 0.0)],
        );
        assert_cut_in(
            &chunk,
            (700, 2_200),
            &limits(500, 1_000, 0),
            &[(1_500, 1_530)],
        );
    }
}

#[test]
fn a_very_long_span_is_cut_at_each_of_its_quiet_stretches() {
    for (rate, channels) in INPUT_FORMATS {
        let dips = [(900, 930, 0.0), (1_800, 1_830, 0.0), (2_650, 2_680, 0.0)];
        let chunk = loud_with_dips(rate, channels, 3_400, &dips);
        assert_cut_in(
            &chunk,
            (0, 3_400),
            &limits(800, 1_000, 0),
            &[(900, 930), (1_800, 1_830), (2_650, 2_680)],
        );
    }
}

#[test]
fn parts_of_a_cut_span_keep_the_span_sample_rate_and_stay_inside_it() {
    for (rate, channels) in INPUT_FORMATS {
        let chunk = ramp(rate, channels, ms_to_frames(5_000, rate));
        let long = span(ms_to_frames(400, rate), ms_to_frames(4_700, rate), rate);
        let pieces =
            segment_speech(&chunk, std::slice::from_ref(&long), &limits(600, 800, 0)).unwrap();
        assert!(pieces.len() >= 6);
        assert_eq!(pieces[0].start_frame, long.start_frame);
        assert_eq!(pieces[pieces.len() - 1].end_frame, long.end_frame);
        for pair in pieces.windows(2) {
            assert_eq!(
                pair[0].end_frame, pair[1].start_frame,
                "continuous speech has no holes"
            );
        }
    }
}
