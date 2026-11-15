mod support;

use audio::{VoiceDetectionConfig, VoiceDetector};
use dictation_types::AudioChunk;
use support::{
    INPUT_FORMATS, Noise, SCORER_FORMATS, ScriptedScorer, assert_spans_valid, bounds, bursts,
    detect_scripted, frame_count, frame_ms, interleave, plain_config, square,
};

fn detect_by_loudness(
    chunk: &AudioChunk,
    scorer_format: (u32, usize),
    config: VoiceDetectionConfig,
) -> Vec<audio::SpeechSpan> {
    let (scorer, _) = ScriptedScorer::by_loudness(scorer_format.0, scorer_format.1, 0.05);
    let spans = VoiceDetector::new(config, scorer, crate::support::resample_tuning())
        .unwrap()
        .detect(chunk)
        .unwrap();
    assert_spans_valid(&spans, chunk);
    spans
}

#[test]
fn a_recording_shorter_than_one_scorer_frame_that_is_all_speech_is_one_span() {
    for (rate, len) in SCORER_FORMATS {
        for frames in [1, 2, len / 2, len - 1] {
            for channels in [1, 2] {
                let chunk = interleave(&square(frames, 0.5), rate, channels);
                let (scorer, log) = ScriptedScorer::constant(rate, len, 1.0);
                let spans =
                    VoiceDetector::new(plain_config(), scorer, crate::support::resample_tuning())
                        .unwrap()
                        .detect(&chunk)
                        .unwrap();
                assert_spans_valid(&spans, &chunk);
                assert_eq!(
                    bounds(&spans),
                    [(0, frames)],
                    "{frames} frames at {rate} Hz"
                );
                assert_eq!(support::frames_seen(&log).len(), 1);
            }
        }
    }
}

#[test]
fn a_recording_shorter_than_one_scorer_frame_in_another_format_is_handled() {
    for (rate, channels) in INPUT_FORMATS {
        for scorer_format in SCORER_FORMATS {
            for frames in [1usize, 3, 17, 100] {
                let chunk = interleave(&square(frames, 0.5), rate, channels);
                for score in [0.0, 1.0] {
                    let (scorer, _) =
                        ScriptedScorer::constant(scorer_format.0, scorer_format.1, score);
                    let spans = VoiceDetector::new(
                        plain_config(),
                        scorer,
                        crate::support::resample_tuning(),
                    )
                    .unwrap()
                    .detect(&chunk)
                    .unwrap();
                    assert_spans_valid(&spans, &chunk);
                    let expected: &[(usize, usize)] =
                        if score > 0.0 { &[(0, frames)] } else { &[] };
                    assert_eq!(
                        bounds(&spans),
                        expected,
                        "{frames} frames at {rate} Hz x{channels}"
                    );
                }
            }
        }
    }
}

#[test]
fn a_stereo_recording_with_a_stray_last_sample_is_handled() {
    let mut chunk = interleave(&square(1_000, 0.5), 16_000, 2);
    chunk.samples.push(0.5);
    let (scorer, _) = ScriptedScorer::constant(16_000, 256, 1.0);
    let spans = VoiceDetector::new(plain_config(), scorer, crate::support::resample_tuning())
        .unwrap()
        .detect(&chunk)
        .unwrap();
    assert_eq!(bounds(&spans), [(0, 1_000)]);
}

#[test]
fn a_recording_that_is_all_speech_is_one_span_from_its_first_frame() {
    for (rate, channels) in INPUT_FORMATS {
        for scorer_format in SCORER_FORMATS {
            for pad_ms in [0, 200] {
                let chunk = bursts(rate, channels, 1_500, &[(0, 1_500)]);
                let config = VoiceDetectionConfig {
                    min_silence_ms: 100,
                    min_speech_ms: 100,
                    pad_ms,
                    ..plain_config()
                };
                let spans = detect_by_loudness(&chunk, scorer_format, config);
                let what = format!(
                    "{rate} Hz x{channels} scored at {} Hz: {spans:?}",
                    scorer_format.0
                );
                assert_eq!(spans.len(), 1, "{what}");
                assert_eq!(spans[0].start_frame, 0, "{what}");
                let tolerance = u64::from(frame_ms(scorer_format));
                assert!(spans[0].end_ms().abs_diff(1_500) <= tolerance, "{what}");
            }
        }
    }
}

#[test]
fn a_recording_that_is_all_speech_in_the_scorer_format_is_covered_to_its_last_frame() {
    for (rate, len) in SCORER_FORMATS {
        let total = 20 * len + len / 3;
        let chunk = interleave(&square(total, 0.5), rate, 1);
        let (scorer, _) = ScriptedScorer::constant(rate, len, 1.0);
        let spans = VoiceDetector::new(plain_config(), scorer, crate::support::resample_tuning())
            .unwrap()
            .detect(&chunk)
            .unwrap();
        assert_eq!(bounds(&spans), [(0, total)]);
    }
}

#[test]
fn speech_only_at_the_very_start_or_the_very_end_is_found_there() {
    for (rate, channels) in INPUT_FORMATS {
        for scorer_format in SCORER_FORMATS {
            let tolerance = u64::from(frame_ms(scorer_format));
            let what = format!("{rate} Hz x{channels} scored at {} Hz", scorer_format.0);
            let config = VoiceDetectionConfig {
                min_silence_ms: 100,
                ..plain_config()
            };

            let at_start = bursts(rate, channels, 2_000, &[(0, 300)]);
            let spans = detect_by_loudness(&at_start, scorer_format, config.clone());
            assert_eq!(spans.len(), 1, "{what}: {spans:?}");
            assert_eq!(spans[0].start_frame, 0, "{what}");
            assert!(
                spans[0].end_ms().abs_diff(300) <= tolerance,
                "{what}: {spans:?}"
            );

            let at_end = bursts(rate, channels, 2_000, &[(1_700, 2_000)]);
            let spans = detect_by_loudness(&at_end, scorer_format, config);
            assert_eq!(spans.len(), 1, "{what}: {spans:?}");
            assert!(
                spans[0].start_ms().abs_diff(1_700) <= tolerance,
                "{what}: {spans:?}"
            );
            assert!(
                spans[0].end_ms().abs_diff(2_000) <= tolerance,
                "{what}: {spans:?}"
            );
            assert!(spans[0].end_frame <= frame_count(&at_end));
        }
    }
}

#[test]
fn many_short_bursts_are_each_found_or_joined_as_the_required_silence_says() {
    let placed: Vec<(u64, u64)> = (0..12).map(|n| (100 + n * 200, 200 + n * 200)).collect();
    for (rate, channels) in INPUT_FORMATS {
        for scorer_format in SCORER_FORMATS {
            let chunk = bursts(rate, channels, 2_600, &placed);
            let what = format!("{rate} Hz x{channels} scored at {} Hz", scorer_format.0);
            let tolerance = u64::from(frame_ms(scorer_format));

            let apart = VoiceDetectionConfig {
                min_silence_ms: 40,
                ..plain_config()
            };
            let spans = detect_by_loudness(&chunk, scorer_format, apart);
            assert_eq!(spans.len(), placed.len(), "{what}: {spans:?}");
            for (span, (start_ms, end_ms)) in spans.iter().zip(&placed) {
                assert!(
                    span.start_ms().abs_diff(*start_ms) <= tolerance,
                    "{what}: {span:?}"
                );
                assert!(
                    span.end_ms().abs_diff(*end_ms) <= tolerance,
                    "{what}: {span:?}"
                );
            }

            // The pauses last 100 ms: asking for 250 ms of silence joins everything.
            let joined = VoiceDetectionConfig {
                min_silence_ms: 250,
                ..plain_config()
            };
            let spans = detect_by_loudness(&chunk, scorer_format, joined);
            assert_eq!(spans.len(), 1, "{what}: {spans:?}");
            assert!(spans[0].start_ms().abs_diff(100) <= tolerance, "{what}");
            assert!(spans[0].end_ms().abs_diff(2_400) <= tolerance, "{what}");
        }
    }
}

#[test]
fn single_frames_alternating_between_speech_and_silence_are_each_their_own_span() {
    for format in SCORER_FORMATS {
        let len = format.1;
        let scores: Vec<f32> = (0..60)
            .map(|n| if n % 2 == 1 { 1.0 } else { 0.0 })
            .collect();
        let expected: Vec<(usize, usize)> = (0..30)
            .map(|n| ((2 * n + 1) * len, (2 * n + 2) * len))
            .collect();
        assert_eq!(detect_scripted(format, &scores, plain_config()), expected);
    }
}

#[test]
fn with_all_durations_at_zero_every_scored_frame_stands_as_it_is() {
    for format in SCORER_FORMATS {
        let len = format.1;
        let scores = [1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0];
        assert_eq!(
            detect_scripted(format, &scores, plain_config()),
            [(0, len), (2 * len, 4 * len), (6 * len, 7 * len)]
        );
    }
}

#[test]
fn a_required_silence_longer_than_the_recording_joins_first_to_last_speech() {
    for format in SCORER_FORMATS {
        let len = format.1;
        let mut scores = vec![0.0; 300];
        for index in [7, 8, 100, 250] {
            scores[index] = 1.0;
        }
        for min_silence_ms in [60_000, u32::MAX] {
            let config = VoiceDetectionConfig {
                min_silence_ms,
                ..plain_config()
            };
            assert_eq!(
                detect_scripted(format, &scores, config),
                [(7 * len, 251 * len)]
            );
        }
    }
}

#[test]
fn a_minimum_speech_duration_longer_than_the_recording_leaves_nothing() {
    for format in SCORER_FORMATS {
        for min_speech_ms in [60_000, u32::MAX] {
            let config = VoiceDetectionConfig {
                min_speech_ms,
                ..plain_config()
            };
            assert_eq!(detect_scripted(format, &vec![1.0; 300], config), []);
        }
    }
}

#[test]
fn padding_longer_than_the_recording_keeps_all_of_it_once_speech_is_found() {
    for format in SCORER_FORMATS {
        let len = format.1;
        let mut scores = vec![0.0; 300];
        scores[40] = 1.0;
        scores[200] = 1.0;
        for pad_ms in [60_000, u32::MAX] {
            let config = VoiceDetectionConfig {
                pad_ms,
                ..plain_config()
            };
            assert_eq!(
                detect_scripted(format, &scores, config.clone()),
                [(0, 300 * len)]
            );
            assert_eq!(detect_scripted(format, &vec![0.0; 300], config), []);
        }
    }
}

#[test]
fn thresholds_at_the_ends_of_the_scale_behave_as_stated() {
    for format in SCORER_FORMATS {
        let len = format.1;
        let scores = [0.0, 0.999, 1.0, 0.999, 0.0];
        let strict = VoiceDetectionConfig {
            speech_threshold: 1.0,
            silence_threshold: 1.0,
            ..plain_config()
        };
        assert_eq!(
            detect_scripted(format, &scores, strict),
            [(2 * len, 3 * len)]
        );
    }
}

#[test]
fn spans_are_well_formed_for_any_scores_and_any_tuning() {
    let mut noise = Noise::new(2_024);
    for round in 0..300 {
        let (rate, channels) = INPUT_FORMATS[noise.below(INPUT_FORMATS.len())];
        let scorer_format = SCORER_FORMATS[noise.below(SCORER_FORMATS.len())];
        let frames = 1 + noise.below(rate as usize / 2);
        let mono: Vec<f32> = (0..frames).map(|_| noise.sample()).collect();
        let chunk = interleave(&mono, rate, channels);

        let speech_threshold = noise.unit();
        let config = VoiceDetectionConfig {
            speech_threshold,
            silence_threshold: speech_threshold * noise.unit(),
            min_loudness: if round % 3 == 0 {
                noise.unit() * 0.8
            } else {
                0.0
            },
            level_to: 0.0,
            level_window_ms: 0,
            min_speech_ms: noise.below(120) as u32,
            min_silence_ms: noise.below(200) as u32,
            pad_ms: noise.below(150) as u32,
        };
        let mut scores = Noise::new(round);
        let stickiness = noise.unit();
        let mut last = 0.0;
        let (scorer, _) = ScriptedScorer::new(scorer_format.0, scorer_format.1, move |_, _| {
            if scores.unit() > stickiness {
                last = scores.unit();
            }
            Ok(last)
        });
        let mut detector =
            VoiceDetector::new(config.clone(), scorer, crate::support::resample_tuning()).unwrap();
        let spans = detector.detect(&chunk).unwrap();
        assert_spans_valid(&spans, &chunk);
        // Spans that touch would have been merged, so there is a gap between any two.
        for pair in spans.windows(2) {
            assert!(
                pair[0].end_frame < pair[1].start_frame,
                "round {round}: {spans:?} with {config:?}"
            );
        }
    }
}
