//! Nothing here claims that a generated signal is speech.

mod support;

use audio::{FrameScorer, NeuralSpeechScorer, VoiceDetectionConfig, VoiceDetector};
use support::{INPUT_FORMATS, Noise, assert_spans_valid, interleave, plain_config, silence};

fn score_all(scorer: &mut NeuralSpeechScorer, samples: &[f32]) -> Vec<f32> {
    let len = scorer.frame_len();
    samples
        .chunks_exact(len)
        .map(|frame| scorer.score(frame).unwrap())
        .collect()
}

fn assert_likelihoods(scores: &[f32], what: &str) {
    assert!(!scores.is_empty(), "{what}: nothing was scored");
    for (n, score) in scores.iter().enumerate() {
        assert!(
            score.is_finite() && (0.0..=1.0).contains(score),
            "{what}: frame {n} scored {score}"
        );
    }
}

fn tone(hz: f32, amplitude: f32, rate: u32, frames: usize) -> Vec<f32> {
    (0..frames)
        .map(|n| amplitude * (2.0 * std::f32::consts::PI * hz * n as f32 / rate as f32).sin())
        .collect()
}

fn noise(seed: u64, amplitude: f32, frames: usize) -> Vec<f32> {
    let mut source = Noise::new(seed);
    (0..frames).map(|_| amplitude * source.sample()).collect()
}

fn mixture(rate: u32) -> Vec<f32> {
    let part = rate as usize / 2;
    let mut samples = tone(220.0, 0.6, rate, part);
    samples.extend(noise(5, 0.4, part));
    samples.extend(tone(1_300.0, 0.9, rate, part));
    samples.extend(vec![0.0; part]);
    samples
}

fn permissive() -> VoiceDetectionConfig {
    VoiceDetectionConfig {
        min_silence_ms: 100,
        ..plain_config()
    }
}

#[test]
fn it_declares_a_format_a_detector_can_use() {
    let scorer = NeuralSpeechScorer::new();
    assert!(scorer.sample_rate_hz() > 0);
    assert!(scorer.frame_len() > 0);
    // A frame is a short piece of audio: well under a second.
    assert!((scorer.frame_len() as u64) * 10 < u64::from(scorer.sample_rate_hz()));
    assert!(VoiceDetector::new(permissive(), scorer, crate::support::resample_tuning()).is_ok());
}

#[test]
fn silence_scores_as_a_likelihood_and_never_as_speech() {
    let mut scorer = NeuralSpeechScorer::new();
    let rate = scorer.sample_rate_hz() as usize;
    let scores = score_all(&mut scorer, &vec![0.0; 10 * rate]);
    assert_likelihoods(&scores, "silence");
    let highest = scores.iter().copied().fold(0.0, f32::max);
    assert!(highest < 0.5, "silence scored up to {highest}");
}

#[test]
fn a_detector_finds_no_speech_in_silence_in_any_format() {
    for (rate, channels) in INPUT_FORMATS {
        let chunk = silence(rate, channels, 3 * rate as usize);
        let mut detector = VoiceDetector::new(
            permissive(),
            NeuralSpeechScorer::new(),
            crate::support::resample_tuning(),
        )
        .unwrap();
        let spans = detector.detect(&chunk).unwrap();
        assert!(spans.is_empty(), "{rate} Hz x{channels}: {spans:?}");
    }
}

#[test]
fn a_detector_copes_with_clipped_and_non_finite_recordings_in_any_format() {
    for (rate, channels) in INPUT_FORMATS {
        let mut mono = tone(500.0, 6.0, rate, rate as usize);
        mono.extend(noise(3, 0.5, rate as usize));
        for (n, slot) in mono.iter_mut().enumerate() {
            match n % 97 {
                0 => *slot = f32::NAN,
                1 => *slot = f32::INFINITY,
                2 => *slot = f32::NEG_INFINITY,
                _ => {}
            }
        }
        let chunk = interleave(&mono, rate, channels);
        for config in [
            permissive(),
            VoiceDetectionConfig {
                min_loudness: 0.05,
                level_to: 0.0,
                level_window_ms: 0,
                pad_ms: 80,
                ..permissive()
            },
        ] {
            let mut detector = VoiceDetector::new(
                config,
                NeuralSpeechScorer::new(),
                crate::support::resample_tuning(),
            )
            .unwrap();
            let spans = detector.detect(&chunk).unwrap();
            assert_spans_valid(&spans, &chunk);
        }
    }
}

#[test]
fn a_reset_scorer_answers_like_a_new_one() {
    let mut fresh = NeuralSpeechScorer::new();
    let rate = fresh.sample_rate_hz();
    let samples = mixture(rate);
    let expected = score_all(&mut fresh, &samples);

    let mut used = NeuralSpeechScorer::default();
    score_all(&mut used, &noise(42, 0.9, 3 * rate as usize));
    score_all(&mut used, &tone(180.0, 0.7, rate, rate as usize));
    used.reset();
    assert_eq!(score_all(&mut used, &samples), expected);
}

#[test]
fn a_detector_gives_the_same_spans_for_the_same_recording_every_time() {
    for (rate, channels) in INPUT_FORMATS {
        let chunk = interleave(&mixture(rate), rate, channels);
        let other = interleave(&noise(8, 0.8, rate as usize), rate, channels);
        let mut detector = VoiceDetector::new(
            permissive(),
            NeuralSpeechScorer::new(),
            crate::support::resample_tuning(),
        )
        .unwrap();
        let first = detector.detect(&chunk).unwrap();
        detector.detect(&other).unwrap();
        let again = detector.detect(&chunk).unwrap();
        assert_spans_valid(&first, &chunk);
        assert_eq!(first, again, "{rate} Hz x{channels}");
    }
}
