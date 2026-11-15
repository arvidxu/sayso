use audio::SpeechSpan;
use dictation_types::AudioChunk;

use super::{frame_count, ms_to_frames, ramp_value, span};

/// Expects pieces cut from a `ramp` recording.
pub fn assert_pieces_faithful(
    chunk: &AudioChunk,
    spans: &[SpeechSpan],
    config: &audio::SegmentationConfig,
    pieces: &[audio::AudioSegment],
) {
    let rate = chunk.sample_rate_hz;
    let frames = frame_count(chunk);
    let gap = ms_to_frames(u64::from(config.gap_ms), rate);

    let mut speech = Vec::new();
    let mut previous_end = 0;
    for (position, piece) in pieces.iter().enumerate() {
        let what = format!("piece {position} of {} at {rate} Hz", pieces.len());
        assert_eq!(piece.index, position, "{what}");
        assert_eq!(
            (piece.audio.sample_rate_hz, piece.audio.channels),
            (rate, 1),
            "{what}"
        );
        assert!(
            piece.start_frame < piece.end_frame && piece.end_frame <= frames,
            "{what}"
        );
        assert!(
            piece.start_frame >= previous_end,
            "{what} starts before the previous one ended"
        );
        previous_end = piece.end_frame;
        assert_eq!(
            piece.start_ms(),
            piece.start_frame as u64 * 1_000 / u64::from(rate),
            "{what}"
        );
        assert_eq!(
            piece.end_ms(),
            piece.end_frame as u64 * 1_000 / u64::from(rate),
            "{what}"
        );
        assert_eq!(
            piece.duration_ms(),
            piece.audio.samples.len() as u64 * 1_000 / u64::from(rate)
        );

        let samples = &piece.audio.samples;
        assert!(
            samples.len() as u128 * 1_000 <= u128::from(config.max_segment_ms) * u128::from(rate),
            "{what} holds {} frames, more than {} ms",
            samples.len(),
            config.max_segment_ms
        );
        assert_eq!(
            samples.first().copied(),
            Some(ramp_value(piece.start_frame)),
            "{what}"
        );
        assert_eq!(
            samples.last().copied(),
            Some(ramp_value(piece.end_frame - 1)),
            "{what}"
        );

        let mut rebuilt = Vec::new();
        let mut inner_end = piece.start_frame;
        assert!(!piece.spans.is_empty(), "{what} lists no speech");
        for (n, part) in piece.spans.iter().enumerate() {
            assert!(
                part.start_frame < part.end_frame,
                "{what} lists an empty stretch"
            );
            assert!(
                part.start_frame >= inner_end && part.end_frame <= piece.end_frame,
                "{what}"
            );
            assert_eq!(part.sample_rate_hz, rate, "{what}");
            let runs_on = n > 0 && part.start_frame == inner_end;
            inner_end = part.end_frame;
            if n > 0 && !runs_on {
                rebuilt.resize(rebuilt.len() + gap, 0.0);
            }
            rebuilt.extend((part.start_frame..part.end_frame).map(ramp_value));
        }
        assert_eq!(piece.spans[0].start_frame, piece.start_frame, "{what}");
        assert_eq!(inner_end, piece.end_frame, "{what}");
        assert!(
            samples == &rebuilt,
            "{what} does not hold its stretches joined by {gap} silent frames"
        );

        speech.extend(samples.iter().copied().filter(|sample| *sample != 0.0));
    }

    let expected: Vec<f32> = spans
        .iter()
        .flat_map(|span| (span.start_frame..span.end_frame).map(ramp_value))
        .collect();
    assert_eq!(
        speech.len(),
        expected.len(),
        "speech was lost or duplicated at {rate} Hz"
    );
    assert!(
        speech == expected,
        "the pieces do not hold the speech of the spans in order"
    );
}

pub fn spans_ms(sample_rate_hz: u32, spans: &[(u64, u64)]) -> Vec<SpeechSpan> {
    spans
        .iter()
        .map(|(start, end)| {
            span(
                ms_to_frames(*start, sample_rate_hz),
                ms_to_frames(*end, sample_rate_hz),
                sample_rate_hz,
            )
        })
        .collect()
}
