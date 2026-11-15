//! Frame `n` covers exactly samples `n * frame_len .. (n + 1) * frame_len`.

mod support;

use audio::VoiceDetectionConfig;
use support::{SCORER_FORMATS, detect_scripted, frame_ms, plain_config};

/// Thresholds apart from each other: 0.5 opens a span, 0.25 keeps it open.
fn two_thresholds() -> VoiceDetectionConfig {
    VoiceDetectionConfig {
        speech_threshold: 0.5,
        silence_threshold: 0.25,
        ..plain_config()
    }
}

fn run(score: f32, count: usize) -> Vec<f32> {
    vec![score; count]
}

fn script(parts: &[Vec<f32>]) -> Vec<f32> {
    parts.concat()
}

#[test]
fn a_score_at_the_speech_threshold_opens_a_span() {
    for format in SCORER_FORMATS {
        let len = format.1;
        let scores = script(&[run(0.0, 3), run(0.5, 1), run(0.0, 3)]);
        assert_eq!(
            detect_scripted(format, &scores, two_thresholds()),
            [(3 * len, 4 * len)]
        );
    }
}

#[test]
fn a_score_just_below_the_speech_threshold_opens_nothing() {
    for format in SCORER_FORMATS {
        let scores = script(&[run(0.0, 3), run(0.499, 20), run(0.0, 3)]);
        assert_eq!(detect_scripted(format, &scores, two_thresholds()), []);
    }
}

#[test]
fn scores_between_the_thresholds_keep_a_span_open_but_never_open_one() {
    for format in SCORER_FORMATS {
        let len = format.1;
        let scores = script(&[run(0.3, 5), run(0.9, 1), run(0.3, 6), run(0.0, 4)]);
        assert_eq!(
            detect_scripted(format, &scores, two_thresholds()),
            [(5 * len, 12 * len)]
        );
    }
}

#[test]
fn a_score_at_the_silence_threshold_keeps_a_span_open() {
    for format in SCORER_FORMATS {
        let len = format.1;
        let scores = script(&[run(0.5, 1), run(0.25, 4), run(0.0, 3)]);
        assert_eq!(
            detect_scripted(format, &scores, two_thresholds()),
            [(0, 5 * len)]
        );
    }
}

#[test]
fn a_score_below_the_silence_threshold_ends_a_span_when_no_silence_is_required() {
    for format in SCORER_FORMATS {
        let len = format.1;
        let scores = script(&[run(0.5, 2), run(0.24, 1), run(0.5, 2)]);
        assert_eq!(
            detect_scripted(format, &scores, two_thresholds()),
            [(0, 2 * len), (3 * len, 5 * len)]
        );
    }
}

#[test]
fn a_span_does_not_include_the_silence_that_ended_it() {
    for format in SCORER_FORMATS {
        let len = format.1;
        let config = VoiceDetectionConfig {
            min_silence_ms: 3 * frame_ms(format),
            ..two_thresholds()
        };
        let scores = script(&[run(0.0, 2), run(1.0, 4), run(0.0, 10)]);
        assert_eq!(
            detect_scripted(format, &scores, config),
            [(2 * len, 6 * len)]
        );
    }
}

#[test]
fn a_dip_shorter_than_the_required_silence_does_not_split_a_span() {
    for format in SCORER_FORMATS {
        let len = format.1;
        let config = VoiceDetectionConfig {
            min_silence_ms: 3 * frame_ms(format),
            ..two_thresholds()
        };
        for dip in [1, 2] {
            let scores = script(&[run(1.0, 4), run(0.0, dip), run(1.0, 4)]);
            assert_eq!(
                detect_scripted(format, &scores, config.clone()),
                [(0, (8 + dip) * len)],
                "a dip of {dip} frames"
            );
        }
    }
}

#[test]
fn silence_of_at_least_the_required_duration_splits_a_span() {
    for format in SCORER_FORMATS {
        let len = format.1;
        let config = VoiceDetectionConfig {
            min_silence_ms: 3 * frame_ms(format),
            ..two_thresholds()
        };
        for dip in [3, 4, 9] {
            let scores = script(&[run(1.0, 4), run(0.0, dip), run(1.0, 4)]);
            assert_eq!(
                detect_scripted(format, &scores, config.clone()),
                [(0, 4 * len), ((4 + dip) * len, (8 + dip) * len)],
                "a dip of {dip} frames"
            );
        }
    }
}

#[test]
fn a_required_silence_between_two_frame_counts_needs_the_larger_one() {
    for format in SCORER_FORMATS {
        let len = format.1;
        let config = VoiceDetectionConfig {
            min_silence_ms: 2 * frame_ms(format) + 1,
            ..two_thresholds()
        };
        let short = script(&[run(1.0, 3), run(0.0, 2), run(1.0, 3)]);
        assert_eq!(
            detect_scripted(format, &short, config.clone()),
            [(0, 8 * len)]
        );
        let long = script(&[run(1.0, 3), run(0.0, 3), run(1.0, 3)]);
        assert_eq!(
            detect_scripted(format, &long, config),
            [(0, 3 * len), (6 * len, 9 * len)]
        );
    }
}

#[test]
fn the_silence_that_ends_a_span_must_be_continuous() {
    for format in SCORER_FORMATS {
        let len = format.1;
        let config = VoiceDetectionConfig {
            min_silence_ms: 3 * frame_ms(format),
            ..two_thresholds()
        };
        // Five quiet frames in all, but a frame at the silence threshold interrupts them.
        let scores = script(&[
            run(1.0, 2),
            run(0.0, 2),
            run(0.25, 1),
            run(0.0, 2),
            run(1.0, 2),
            run(0.0, 6),
        ]);
        assert_eq!(detect_scripted(format, &scores, config), [(0, 9 * len)]);
    }
}

#[test]
fn a_span_still_open_at_the_end_of_the_recording_is_reported() {
    for format in SCORER_FORMATS {
        let len = format.1;
        let config = VoiceDetectionConfig {
            min_silence_ms: 5 * frame_ms(format),
            ..two_thresholds()
        };
        let to_the_end = script(&[run(0.0, 2), run(1.0, 3)]);
        assert_eq!(
            detect_scripted(format, &to_the_end, config.clone()),
            [(2 * len, 5 * len)]
        );
        let short_tail = script(&[run(0.0, 2), run(1.0, 3), run(0.0, 2)]);
        assert_eq!(
            detect_scripted(format, &short_tail, config),
            [(2 * len, 5 * len)]
        );
    }
}

#[test]
fn a_span_exactly_as_long_as_the_minimum_is_kept() {
    for format in SCORER_FORMATS {
        let len = format.1;
        let config = VoiceDetectionConfig {
            min_speech_ms: 5 * frame_ms(format),
            ..plain_config()
        };
        let scores = script(&[run(0.0, 2), run(1.0, 5), run(0.0, 2)]);
        assert_eq!(
            detect_scripted(format, &scores, config),
            [(2 * len, 7 * len)]
        );
    }
}

#[test]
fn spans_shorter_than_the_minimum_are_dropped() {
    for format in SCORER_FORMATS {
        let len = format.1;
        let exact = 5 * frame_ms(format);
        let scores = script(&[run(0.0, 2), run(1.0, 5), run(0.0, 2)]);
        let one_more = VoiceDetectionConfig {
            min_speech_ms: exact + 1,
            ..plain_config()
        };
        assert_eq!(detect_scripted(format, &scores, one_more), []);

        // Short bursts go, the long one stays, wherever they are in the recording.
        let config = VoiceDetectionConfig {
            min_speech_ms: exact,
            ..plain_config()
        };
        let mixed = script(&[
            run(1.0, 4),
            run(0.0, 3),
            run(1.0, 6),
            run(0.0, 3),
            run(1.0, 1),
            run(0.0, 3),
            run(1.0, 4),
        ]);
        assert_eq!(
            detect_scripted(format, &mixed, config),
            [(7 * len, 13 * len)]
        );
    }
}

#[test]
fn padding_keeps_audio_before_and_after_a_span() {
    for format in SCORER_FORMATS {
        let (rate, len) = format;
        for pad_ms in [1, 7, frame_ms(format), 45] {
            let pad = (u64::from(pad_ms) * u64::from(rate) / 1_000) as usize;
            let config = VoiceDetectionConfig {
                pad_ms,
                ..plain_config()
            };
            let scores = script(&[run(0.0, 10), run(1.0, 5), run(0.0, 10)]);
            assert_eq!(
                detect_scripted(format, &scores, config),
                [(10 * len - pad, 15 * len + pad)],
                "{pad_ms} ms of padding at {rate} Hz"
            );
        }
    }
}

#[test]
fn padding_never_reaches_beyond_the_recording() {
    for format in SCORER_FORMATS {
        let len = format.1;
        for pad_ms in [3 * frame_ms(format), 1_000, u32::MAX] {
            let config = VoiceDetectionConfig {
                pad_ms,
                ..plain_config()
            };
            let at_start = script(&[run(1.0, 2), run(0.0, 400)]);
            let spans = detect_scripted(format, &at_start, config.clone());
            assert_eq!(spans.len(), 1);
            assert_eq!(spans[0].0, 0);

            let at_end = script(&[run(0.0, 400), run(1.0, 2)]);
            let spans = detect_scripted(format, &at_end, config);
            assert_eq!(spans.len(), 1);
            assert_eq!(spans[0].1, 402 * len);
        }
    }
}

#[test]
fn spans_that_touch_or_overlap_after_padding_become_one() {
    for format in SCORER_FORMATS {
        let (rate, len) = format;
        // Two spans six frames apart.
        let scores = script(&[
            run(0.0, 10),
            run(1.0, 4),
            run(0.0, 6),
            run(1.0, 4),
            run(0.0, 10),
        ]);
        let pad_frames = |frames: u32| (frames * frame_ms(format), frames as usize * len);

        let (pad_ms, pad) = pad_frames(2);
        let apart = detect_scripted(
            format,
            &scores,
            VoiceDetectionConfig {
                pad_ms,
                ..plain_config()
            },
        );
        assert_eq!(
            apart,
            [
                (10 * len - pad, 14 * len + pad),
                (20 * len - pad, 24 * len + pad)
            ]
        );

        for frames in [3, 4, 6] {
            let (pad_ms, pad) = pad_frames(frames);
            let merged = detect_scripted(
                format,
                &scores,
                VoiceDetectionConfig {
                    pad_ms,
                    ..plain_config()
                },
            );
            assert_eq!(
                merged,
                [(10 * len - pad, 24 * len + pad)],
                "{pad_ms} ms of padding at {rate} Hz"
            );
        }
    }
}

#[test]
fn padding_merges_a_whole_row_of_spans() {
    for format in SCORER_FORMATS {
        let len = format.1;
        let scores: Vec<f32> = (0..40)
            .map(|n| if n % 4 == 0 { 1.0 } else { 0.0 })
            .collect();
        let config = VoiceDetectionConfig {
            pad_ms: 2 * frame_ms(format),
            ..plain_config()
        };
        // Bursts at frames 0, 4, ... 36, each padded by two frames on both sides.
        assert_eq!(detect_scripted(format, &scores, config), [(0, 39 * len)]);
    }
}
