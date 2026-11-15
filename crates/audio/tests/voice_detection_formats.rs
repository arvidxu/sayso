mod support;

use audio::{VoiceDetectionConfig, VoiceDetector};
use dictation_types::{AppError, AudioChunk};
use support::{
    Event, INPUT_FORMATS, SCORER_FORMATS, ScriptedScorer, assert_spans_valid, bounds, bursts,
    frame_count, frame_ms, frames_seen, interleave, plain_config, silence, square,
};

/// Where the stand-in speech is, in milliseconds, in a recording of `TOTAL_MS`.
const PLACED: [(u64, u64); 2] = [(300, 900), (1_400, 1_800)];
const TOTAL_MS: u64 = 2_200;

fn assert_within(actual_ms: u64, expected_ms: u64, tolerance_ms: u64, what: &str) {
    assert!(
        actual_ms.abs_diff(expected_ms) <= tolerance_ms,
        "{what}: {actual_ms} ms is more than {tolerance_ms} ms away from {expected_ms} ms"
    );
}

#[test]
fn spans_are_in_frames_of_the_input_and_sit_where_the_speech_was_placed() {
    for (rate, channels) in INPUT_FORMATS {
        for scorer_format in SCORER_FORMATS {
            for pad_ms in [0u32, 30] {
                let what = format!(
                    "{rate} Hz x{channels} scored at {} Hz with {pad_ms} ms padding",
                    scorer_format.0
                );
                let chunk = bursts(rate, channels, TOTAL_MS, &PLACED);
                let (scorer, _) =
                    ScriptedScorer::by_loudness(scorer_format.0, scorer_format.1, 0.05);
                let config = VoiceDetectionConfig {
                    min_silence_ms: 100,
                    pad_ms,
                    ..plain_config()
                };
                let mut detector =
                    VoiceDetector::new(config, scorer, crate::support::resample_tuning()).unwrap();
                let spans = detector.detect(&chunk).unwrap();
                assert_spans_valid(&spans, &chunk);
                assert_eq!(spans.len(), PLACED.len(), "{what}: {spans:?}");

                let tolerance = u64::from(frame_ms(scorer_format) + pad_ms);
                for (span, (start_ms, end_ms)) in spans.iter().zip(PLACED) {
                    assert_eq!(span.sample_rate_hz, rate, "{what}");
                    assert_within(span.start_ms(), start_ms, tolerance, &what);
                    assert_within(span.end_ms(), end_ms, tolerance, &what);
                    // The positions in frames say the same as the positions in milliseconds.
                    let start_frame_ms = span.start_frame as u64 * 1_000 / u64::from(rate);
                    let end_frame_ms = span.end_frame as u64 * 1_000 / u64::from(rate);
                    assert_eq!(
                        (span.start_ms(), span.end_ms()),
                        (start_frame_ms, end_frame_ms)
                    );
                    assert_eq!(
                        span.duration_ms(),
                        span.frames() as u64 * 1_000 / u64::from(rate)
                    );
                }
            }
        }
    }
}

#[test]
fn the_span_holds_the_loud_part_of_the_input_itself() {
    // Cutting the input at the reported frames must give the placed tone, not silence.
    for (rate, channels) in INPUT_FORMATS {
        let chunk = bursts(rate, channels, TOTAL_MS, &PLACED);
        let (scorer, _) = ScriptedScorer::by_loudness(16_000, 256, 0.05);
        let config = VoiceDetectionConfig {
            min_silence_ms: 100,
            ..plain_config()
        };
        let spans = VoiceDetector::new(config, scorer, crate::support::resample_tuning())
            .unwrap()
            .detect(&chunk)
            .unwrap();
        let kept = audio::keep_speech(&chunk, &spans).unwrap();
        let placed_ms: u64 = PLACED.iter().map(|(start, end)| end - start).sum();
        assert!(
            kept.duration_ms().abs_diff(placed_ms) <= 4 * 16,
            "{rate} Hz x{channels}"
        );
        let level = support::rms(&kept.samples);
        assert!(
            level > 0.3,
            "{rate} Hz x{channels}: kept audio has loudness {level}"
        );
    }
}

#[test]
fn a_frame_below_the_loudness_floor_is_never_speech_whatever_the_scorer_says() {
    for (rate, len) in SCORER_FORMATS {
        // Ten frames each: silence, loud, very quiet, loud, silence. The scorer says "speech"
        // for all of them.
        let mut mono = vec![0.0; 10 * len];
        mono.extend(square(10 * len, 0.5));
        mono.extend(square(10 * len, 0.01));
        mono.extend(square(10 * len, 0.5));
        mono.extend(vec![0.0; 10 * len]);
        for channels in [1, 2] {
            let chunk = interleave(&mono, rate, channels);
            let (scorer, _) = ScriptedScorer::constant(rate, len, 1.0);
            let config = VoiceDetectionConfig {
                min_loudness: 0.05,
                level_to: 0.0,
                level_window_ms: 0,
                ..plain_config()
            };
            let spans = VoiceDetector::new(config, scorer, crate::support::resample_tuning())
                .unwrap()
                .detect(&chunk)
                .unwrap();
            assert_spans_valid(&spans, &chunk);
            assert_eq!(bounds(&spans), [(10 * len, 20 * len), (30 * len, 40 * len)]);
        }
    }
}

#[test]
fn a_frame_exactly_as_loud_as_the_floor_can_be_speech() {
    for (rate, len) in SCORER_FORMATS {
        let chunk = interleave(&vec![0.5; 6 * len], rate, 1);
        let (scorer, _) = ScriptedScorer::constant(rate, len, 1.0);
        let config = VoiceDetectionConfig {
            min_loudness: 0.5,
            level_to: 0.0,
            level_window_ms: 0,
            ..plain_config()
        };
        let spans = VoiceDetector::new(config, scorer, crate::support::resample_tuning())
            .unwrap()
            .detect(&chunk)
            .unwrap();
        assert_eq!(bounds(&spans), [(0, 6 * len)]);
    }
}

#[test]
fn without_a_loudness_floor_the_scorer_alone_decides() {
    for (rate, len) in SCORER_FORMATS {
        let mut mono = vec![0.0; 10 * len];
        mono.extend(square(10 * len, 0.5));
        mono.extend(square(10 * len, 0.000_1));
        let chunk = interleave(&mono, rate, 1);

        let (always, _) = ScriptedScorer::constant(rate, len, 1.0);
        let spans = VoiceDetector::new(plain_config(), always, crate::support::resample_tuning())
            .unwrap()
            .detect(&chunk)
            .unwrap();
        assert_eq!(
            bounds(&spans),
            [(0, 30 * len)],
            "digital silence scored as speech"
        );

        let (never, _) = ScriptedScorer::constant(rate, len, 0.0);
        let spans = VoiceDetector::new(plain_config(), never, crate::support::resample_tuning())
            .unwrap()
            .detect(&chunk)
            .unwrap();
        assert_eq!(bounds(&spans), [], "loud audio scored as silence");
    }
}

#[test]
fn the_scorer_sees_every_frame_once_in_order_with_the_last_one_filled_up() {
    for (rate, len) in SCORER_FORMATS {
        // Three and a half frames, every sample different.
        let total = 3 * len + len / 2;
        let mono: Vec<f32> = (0..total)
            .map(|n| (n + 1) as f32 / (total + 1) as f32)
            .collect();
        let (scorer, log) = ScriptedScorer::constant(rate, len, 0.0);
        let mut detector =
            VoiceDetector::new(plain_config(), scorer, crate::support::resample_tuning()).unwrap();
        detector.detect(&interleave(&mono, rate, 1)).unwrap();

        let mut expected: Vec<Vec<f32>> = mono.chunks(len).map(<[f32]>::to_vec).collect();
        expected.last_mut().unwrap().resize(len, 0.0);
        assert_eq!(expected.len(), 4);
        assert_eq!(frames_seen(&log), expected, "{rate} Hz, frames of {len}");
    }
}

#[test]
fn the_scorer_gets_samples_within_full_scale_and_silence_for_unusable_ones() {
    for (rate, len) in SCORER_FORMATS {
        let mut mono = vec![0.25f32; 2 * len];
        mono[0] = 3.0;
        mono[1] = -7.5;
        mono[2] = f32::NAN;
        mono[3] = f32::INFINITY;
        mono[4] = f32::NEG_INFINITY;
        mono[5] = 1.0;
        mono[6] = -1.0;
        mono[len] = f32::MAX;
        mono[len + 1] = f32::MIN;
        let (scorer, log) = ScriptedScorer::constant(rate, len, 0.0);
        let mut detector =
            VoiceDetector::new(plain_config(), scorer, crate::support::resample_tuning()).unwrap();
        detector.detect(&AudioChunk::new(mono, rate, 1)).unwrap();

        let seen = frames_seen(&log);
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0][..8], [1.0, -1.0, 0.0, 0.0, 0.0, 1.0, -1.0, 0.25]);
        assert_eq!(seen[1][..3], [1.0, -1.0, 0.25]);
    }
}

#[test]
fn the_scorer_gets_whole_clean_frames_for_input_in_any_format() {
    for (rate, channels) in INPUT_FORMATS {
        for (scorer_rate, len) in SCORER_FORMATS {
            let mut chunk = bursts(rate, channels, 700, &[(100, 500)]);
            chunk.samples[10] = f32::NAN;
            chunk.samples[11] = 40.0;
            chunk.samples[12] = f32::NEG_INFINITY;
            let (scorer, log) = ScriptedScorer::constant(scorer_rate, len, 0.0);
            VoiceDetector::new(plain_config(), scorer, crate::support::resample_tuning())
                .unwrap()
                .detect(&chunk)
                .unwrap();

            let seen = frames_seen(&log);
            let what = format!("{rate} Hz x{channels} scored at {scorer_rate} Hz");
            // 700 ms of audio, give or take one frame for the format conversion.
            let expected = (700 * u64::from(scorer_rate) / 1_000).div_ceil(len as u64) as usize;
            assert!(
                seen.len().abs_diff(expected) <= 1,
                "{what}: {} frames",
                seen.len()
            );
            for frame in &seen {
                assert_eq!(frame.len(), len, "{what}");
                assert!(
                    frame
                        .iter()
                        .all(|s| s.is_finite() && (-1.0..=1.0).contains(s)),
                    "{what}: a frame holds a sample the scorer must never see"
                );
            }
        }
    }
}

#[test]
fn the_scorer_is_reset_before_each_recording_and_not_in_between() {
    for (rate, len) in SCORER_FORMATS {
        let (scorer, log) = ScriptedScorer::constant(rate, len, 1.0);
        let mut detector =
            VoiceDetector::new(plain_config(), scorer, crate::support::resample_tuning()).unwrap();
        let chunk = silence(rate, 1, 5 * len);
        detector.detect(&chunk).unwrap();
        detector.detect(&chunk).unwrap();

        let events = log.lock().unwrap().clone();
        let kinds: Vec<bool> = events.iter().map(|event| *event == Event::Reset).collect();
        let mut one_run = vec![true];
        one_run.extend([false; 5]);
        assert_eq!(kinds, [one_run.clone(), one_run].concat());
    }
}

#[test]
fn two_runs_over_the_same_audio_give_the_same_spans() {
    for (rate, channels) in INPUT_FORMATS {
        for (scorer_rate, len) in SCORER_FORMATS {
            // A scorer that only answers by frame number: without a reset the second run
            // would be scored as if the recording went on.
            let (scorer, _) = ScriptedScorer::new(scorer_rate, len, |index, _| {
                Ok(if (5..12).contains(&index) || (20..23).contains(&index) {
                    0.9
                } else {
                    0.1
                })
            });
            let mut detector =
                VoiceDetector::new(plain_config(), scorer, crate::support::resample_tuning())
                    .unwrap();
            let chunk = bursts(rate, channels, 900, &[(0, 900)]);
            let first = detector.detect(&chunk).unwrap();
            let second = detector.detect(&chunk).unwrap();
            assert_eq!(first.len(), 2);
            assert_eq!(
                first, second,
                "{rate} Hz x{channels} scored at {scorer_rate} Hz"
            );
        }
    }
}

#[test]
fn empty_audio_gives_no_spans_and_asks_the_scorer_nothing() {
    for (rate, channels) in INPUT_FORMATS {
        let (scorer, log) = ScriptedScorer::constant(16_000, 256, 1.0);
        let mut detector =
            VoiceDetector::new(plain_config(), scorer, crate::support::resample_tuning()).unwrap();
        let spans = detector
            .detect(&AudioChunk::new(Vec::new(), rate, channels))
            .unwrap();
        assert!(spans.is_empty());
        assert!(frames_seen(&log).is_empty());
    }
}

#[test]
fn audio_without_channels_is_rejected() {
    let (scorer, _) = ScriptedScorer::constant(16_000, 256, 1.0);
    let mut detector =
        VoiceDetector::new(plain_config(), scorer, crate::support::resample_tuning()).unwrap();
    assert!(
        detector
            .detect(&AudioChunk::new(vec![0.5; 4_000], 16_000, 0))
            .is_err()
    );
    assert!(
        detector
            .detect(&AudioChunk::new(Vec::new(), 16_000, 0))
            .is_err()
    );
}

#[test]
fn audio_at_an_unusable_sample_rate_is_rejected() {
    for scorer_format in SCORER_FORMATS {
        let (scorer, _) = ScriptedScorer::constant(scorer_format.0, scorer_format.1, 1.0);
        let mut detector =
            VoiceDetector::new(plain_config(), scorer, crate::support::resample_tuning()).unwrap();
        for channels in [1, 2] {
            let chunk = AudioChunk::new(vec![0.5; 4_000], 0, channels);
            assert!(detector.detect(&chunk).is_err(), "0 Hz x{channels}");
        }
    }
}

#[test]
fn an_error_from_the_scorer_is_passed_on() {
    for (rate, channels) in INPUT_FORMATS {
        for fail_at in [0, 3] {
            let (scorer, _) = ScriptedScorer::new(16_000, 256, move |index, _| {
                if index == fail_at {
                    Err(AppError::audio("the scorer broke"))
                } else {
                    Ok(1.0)
                }
            });
            let mut detector =
                VoiceDetector::new(plain_config(), scorer, crate::support::resample_tuning())
                    .unwrap();
            let chunk = bursts(rate, channels, 500, &[(0, 500)]);
            assert_eq!(
                detector.detect(&chunk),
                Err(AppError::audio("the scorer broke"))
            );
        }
    }
}

#[test]
fn a_detector_works_again_after_its_scorer_failed_once() {
    let (scorer, _) = {
        let mut calls = 0;
        ScriptedScorer::new(8_000, 160, move |_, _| {
            calls += 1;
            if calls == 2 {
                Err(AppError::audio("once"))
            } else {
                Ok(1.0)
            }
        })
    };
    let mut detector =
        VoiceDetector::new(plain_config(), scorer, crate::support::resample_tuning()).unwrap();
    let chunk = silence(8_000, 1, 800);
    assert!(detector.detect(&chunk).is_err());
    let spans = detector.detect(&chunk).unwrap();
    assert_eq!(bounds(&spans), [(0, frame_count(&chunk))]);
}

#[test]
fn quiet_audio_is_turned_up_for_the_scorer_but_loud_audio_is_left_alone() {
    // The scorer reports how loud the frames it is given are, so the test can see them.
    let detector_with = |level_to: f32| {
        let (scorer, calls) = ScriptedScorer::new(16_000, 160, |_, frame| {
            Ok(frame
                .iter()
                .fold(0.0f32, |peak: f32, sample| peak.max(sample.abs())))
        });
        let config = VoiceDetectionConfig {
            speech_threshold: 0.9,
            silence_threshold: 0.5,
            min_loudness: 0.0005,
            level_to,
            level_window_ms: 500,
            min_speech_ms: 0,
            min_silence_ms: 0,
            pad_ms: 0,
        };
        (
            VoiceDetector::new(config, scorer, support::resample_tuning()).unwrap(),
            calls,
        )
    };
    let tone = |peak: f32| AudioChunk::new(vec![peak; 16_000], 16_000, 1);

    // Left as it is: the scorer sees the quiet audio as quiet.
    let (mut plain, _) = detector_with(0.0);
    assert!(plain.detect(&tone(0.002)).unwrap().is_empty());

    // Turned up: the same audio now reaches the scorer at the wanted level and is found.
    let (mut levelled, _) = detector_with(0.95);
    let spans = levelled.detect(&tone(0.002)).unwrap();
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].frames(), 16_000);

    // Never turned down, and never above full scale.
    let (mut loud, _) = detector_with(0.1);
    assert_eq!(loud.detect(&tone(0.95)).unwrap().len(), 1);

    // Below the loudness floor nothing is turned up: silence stays silence.
    let (mut silent, _) = detector_with(0.95);
    assert!(silent.detect(&tone(0.0001)).unwrap().is_empty());
}

#[test]
fn silence_is_not_turned_up_when_there_is_no_loudness_floor() {
    // The scorer calls any frame that is not a number speech, so a broken frame shows.
    let (scorer, _) = ScriptedScorer::new(16_000, 160, |_, frame| {
        Ok(if frame.iter().any(|sample| sample.is_nan()) {
            1.0
        } else {
            0.0
        })
    });
    let config = VoiceDetectionConfig {
        speech_threshold: 0.9,
        silence_threshold: 0.5,
        min_loudness: 0.0,
        level_to: 0.95,
        level_window_ms: 500,
        min_speech_ms: 0,
        min_silence_ms: 0,
        pad_ms: 0,
    };
    let mut detector = VoiceDetector::new(config, scorer, support::resample_tuning()).unwrap();
    let chunk = AudioChunk::new(vec![0.0; 16_000], 16_000, 1);
    assert!(detector.detect(&chunk).unwrap().is_empty());
}
