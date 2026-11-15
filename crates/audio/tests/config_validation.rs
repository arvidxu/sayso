mod support;

use audio::{SegmentationConfig, VoiceDetectionConfig, VoiceDetector, segment_speech};
use support::{ScriptedScorer, plain_config, silence};

fn detector_for(config: VoiceDetectionConfig) -> Result<(), dictation_types::AppError> {
    let (scorer, _) = ScriptedScorer::constant(16_000, 256, 0.0);
    VoiceDetector::new(config, scorer, crate::support::resample_tuning()).map(|_| ())
}

fn assert_vad_rejected(config: VoiceDetectionConfig) {
    assert!(config.validate().is_err(), "validate accepted {config:?}");
    assert!(
        detector_for(config.clone()).is_err(),
        "a detector was built with {config:?}"
    );
}

fn assert_vad_accepted(config: VoiceDetectionConfig) {
    assert!(config.validate().is_ok(), "validate refused {config:?}");
    assert!(
        detector_for(config.clone()).is_ok(),
        "no detector could be built with {config:?}"
    );
}

#[test]
fn a_speech_threshold_not_above_zero_or_above_one_is_rejected() {
    for bad in [
        0.0,
        -0.01,
        1.01,
        -1.0,
        2.0,
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
    ] {
        assert_vad_rejected(VoiceDetectionConfig {
            speech_threshold: bad,
            silence_threshold: 0.0,
            ..plain_config()
        });
    }
}

#[test]
fn a_silence_threshold_outside_zero_to_one_is_rejected() {
    for bad in [
        -0.01,
        1.01,
        -1.0,
        2.0,
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
    ] {
        assert_vad_rejected(VoiceDetectionConfig {
            speech_threshold: 1.0,
            silence_threshold: bad,
            ..plain_config()
        });
    }
}

#[test]
fn a_loudness_floor_outside_zero_to_one_is_rejected() {
    for bad in [
        -0.01,
        1.01,
        -1.0,
        2.0,
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
    ] {
        assert_vad_rejected(VoiceDetectionConfig {
            min_loudness: bad,
            level_to: 0.0,
            level_window_ms: 0,
            ..plain_config()
        });
    }
}

#[test]
fn a_silence_threshold_above_the_speech_threshold_is_rejected() {
    for (speech, silence) in [(0.5, 0.51), (0.01, 0.1), (0.3, 1.0), (0.999, 1.0)] {
        assert_vad_rejected(VoiceDetectionConfig {
            speech_threshold: speech,
            silence_threshold: silence,
            ..plain_config()
        });
    }
}

#[test]
fn equal_thresholds_are_accepted() {
    for both in [0.01, 0.25, 0.5, 1.0] {
        assert_vad_accepted(VoiceDetectionConfig {
            speech_threshold: both,
            silence_threshold: both,
            ..plain_config()
        });
    }
}

#[test]
fn thresholds_and_loudness_at_the_ends_of_the_range_are_accepted() {
    for (speech, silence, loudness) in [
        (1.0, 0.0, 0.0),
        (1.0, 0.0, 1.0),
        (0.01, 0.0, 1.0),
        (0.7, 0.2, 0.3),
    ] {
        assert_vad_accepted(VoiceDetectionConfig {
            speech_threshold: speech,
            silence_threshold: silence,
            min_loudness: loudness,
            level_to: 0.0,
            level_window_ms: 0,
            ..plain_config()
        });
    }
}

#[test]
fn any_duration_is_an_acceptable_tuning_value() {
    for ms in [0, 1, 30_000, u32::MAX] {
        assert_vad_accepted(VoiceDetectionConfig {
            min_speech_ms: ms,
            min_silence_ms: ms,
            pad_ms: ms,
            ..plain_config()
        });
    }
}

#[test]
fn a_scorer_declaring_a_zero_frame_length_is_rejected() {
    let (scorer, _) = ScriptedScorer::constant(16_000, 0, 0.0);
    assert!(VoiceDetector::new(plain_config(), scorer, crate::support::resample_tuning()).is_err());
}

#[test]
fn a_scorer_declaring_a_zero_sample_rate_is_rejected() {
    let (scorer, _) = ScriptedScorer::constant(0, 256, 0.0);
    assert!(VoiceDetector::new(plain_config(), scorer, crate::support::resample_tuning()).is_err());
}

fn limits(target_segment_ms: u64, max_segment_ms: u64) -> SegmentationConfig {
    SegmentationConfig {
        target_segment_ms,
        max_segment_ms,
        gap_ms: 0,
        min_part_ms: (target_segment_ms / 2).max(1),
        quiet_window_ms: 10,
    }
}

#[test]
fn a_target_length_of_zero_is_rejected() {
    for max in [0, 1, 30_000] {
        assert!(limits(0, max).validate().is_err(), "maximum {max}");
    }
}

#[test]
fn a_maximum_below_the_target_is_rejected() {
    for (target, max) in [(2, 1), (1_000, 999), (30_000, 1), (u64::MAX, u64::MAX - 1)] {
        assert!(limits(target, max).validate().is_err(), "{target} / {max}");
    }
}

#[test]
fn a_maximum_equal_to_the_target_is_accepted() {
    for both in [1, 1_000, 30_000, u64::MAX] {
        assert!(limits(both, both).validate().is_ok(), "{both}");
    }
}

#[test]
fn a_maximum_above_the_target_is_accepted_with_any_gap() {
    for gap_ms in [0, 1, 250, u32::MAX] {
        let config = SegmentationConfig {
            target_segment_ms: 1_000,
            max_segment_ms: 2_000,
            gap_ms,
            min_part_ms: 500,
            quiet_window_ms: 10,
        };
        assert!(config.validate().is_ok(), "gap {gap_ms}");
    }
}

#[test]
fn segmenting_refuses_invalid_limits_even_when_there_is_nothing_to_cut() {
    let chunk = silence(16_000, 1, 16_000);
    for config in [limits(0, 1_000), limits(1_000, 999), limits(0, 0)] {
        assert!(
            segment_speech(&chunk, &[], &config).is_err(),
            "segmented with {config:?}"
        );
        let spans = [support::span(0, 8_000, 16_000)];
        assert!(
            segment_speech(&chunk, &spans, &config).is_err(),
            "segmented with {config:?}"
        );
    }
}
