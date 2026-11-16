mod support;

use std::collections::HashSet;

use dictation_api::AsrEngine;
use dictation_types::{ModelDescriptor, Transcript};
use support::{ENGINE, Fixture, options, ramp, speech_entry};

fn model_at(sample_rate_hz: u32) -> ModelDescriptor {
    speech_entry(
        "model-a",
        ENGINE,
        &["hu", "am", "ka", "es", "zu"],
        &[
            ("sampleRateHz", &sample_rate_hz.to_string()),
            ("languageMixing", "mixed"),
        ],
    )
}

fn fixture() -> Fixture {
    Fixture::installed(model_at(16_000))
}

fn sorted_timings(transcript: &Transcript) -> Vec<u64> {
    let mut values: Vec<u64> = transcript.timings.stages.iter().map(|t| t.ms).collect();
    values.sort_unstable();
    values
}

#[test]
fn the_backends_text_becomes_the_transcript_text() {
    let fixture = fixture();
    for text in ["good morning everyone", "habari ya asubuhi", "x"] {
        fixture.backend.answer_next(text);
        let transcript = fixture
            .engine
            .transcribe(&ramp(8_000), options("hu"))
            .unwrap();
        assert_eq!(transcript.raw_text, text);
        assert_eq!(transcript.effective_text(), text);
        assert_eq!(transcript.final_text, None);
        assert!(!transcript.is_partial);
    }
}

#[test]
fn space_around_the_backends_text_is_removed() {
    let fixture = fixture();
    for padded in [
        "  two words",
        "two words \n",
        "\t two words\r\n",
        "\u{3000}two words\u{a0}",
    ] {
        fixture.backend.answer_next(padded);
        let transcript = fixture
            .engine
            .transcribe(&ramp(8_000), options("es"))
            .unwrap();
        assert_eq!(transcript.raw_text, "two words", "{padded:?}");
        assert_eq!(transcript.segments[0].text, "two words", "{padded:?}");
    }
}

#[test]
fn the_text_is_one_final_segment_spanning_the_whole_recording() {
    let fixture = fixture();
    fixture.backend.answer_next("all of it");
    let transcript = fixture
        .engine
        .transcribe(&ramp(40_000), options("ka"))
        .unwrap();
    assert_eq!(transcript.segments.len(), 1);
    let segment = &transcript.segments[0];
    assert_eq!(segment.text, "all of it");
    assert_eq!(segment.start_ms, 0);
    assert_eq!(segment.end_ms, 2_500);
    assert!(segment.is_final);
}

#[test]
fn the_audio_duration_follows_from_the_samples_and_the_sample_rate() {
    let cases = [
        (16_000u32, 24_000usize, 1_500u64),
        (8_000, 4_000, 500),
        (48_000, 144_000, 3_000),
        (22_050, 11_025, 500),
        (16_000, 16, 1),
    ];
    for (rate, samples, expected_ms) in cases {
        let fixture = Fixture::installed(model_at(rate));
        let transcript = fixture
            .engine
            .transcribe(&ramp(samples), options("am"))
            .unwrap();
        assert_eq!(
            transcript.audio_duration_ms, expected_ms,
            "{samples} samples at {rate} Hz"
        );
        assert_eq!(transcript.segments[0].end_ms, expected_ms);
    }
}

#[test]
fn the_transcript_carries_the_languages_that_were_asked_for() {
    let fixture = fixture();
    for spec in ["zu", "ka+hu", "es+am+zu+hu", "hu+ka+es"] {
        let opts = options(spec);
        let transcript = fixture
            .engine
            .transcribe(&ramp(1_600), opts.clone())
            .unwrap();
        assert_eq!(transcript.languages, opts.languages, "{spec}");
    }
}

#[test]
fn the_transcript_says_it_was_recognised_on_this_device_by_this_model() {
    let fixture = fixture();
    let transcript = fixture
        .engine
        .transcribe(&ramp(1_600), options("hu"))
        .unwrap();
    assert_eq!(transcript.request_routes.len(), 1);
    let route = &transcript.request_routes[0];
    assert!(route.is_local());
    assert_eq!(route.endpoint, None);
    assert_eq!(route.provider_id, fixture.engine.id());
    assert_eq!(route.model_id.as_ref(), Some(&fixture.model.id));
    assert!(!route.stage.trim().is_empty());
}

#[test]
fn loading_and_recognising_are_timed_separately() {
    let fixture = fixture();
    fixture.backend.loading_takes_ms(70);
    fixture.backend.recognising_takes_ms(30);

    let first = fixture
        .engine
        .transcribe(&ramp(1_600), options("hu"))
        .unwrap();
    assert_eq!(sorted_timings(&first), [30, 70]);
    let names: HashSet<&str> = first
        .timings
        .stages
        .iter()
        .map(|t| t.stage.as_str())
        .collect();
    assert_eq!(names.len(), 2, "two differently named stages");
}

#[test]
fn no_load_time_is_recorded_when_the_model_was_already_in_memory() {
    let fixture = fixture();
    fixture.backend.loading_takes_ms(70);
    fixture.backend.recognising_takes_ms(30);

    let first = fixture
        .engine
        .transcribe(&ramp(1_600), options("hu"))
        .unwrap();
    let second = fixture
        .engine
        .transcribe(&ramp(1_600), options("hu"))
        .unwrap();
    assert_eq!(sorted_timings(&second), [30]);
    assert_eq!(second.timings.stages.len(), 1);
    let recognition_stage = &second.timings.stages[0].stage;
    assert_eq!(first.timings.get(recognition_stage), Some(30));
}

#[test]
fn no_load_time_is_recorded_when_the_model_was_loaded_ahead_of_time() {
    let fixture = fixture();
    fixture.backend.loading_takes_ms(70);
    fixture.backend.recognising_takes_ms(30);
    fixture.engine.load().unwrap();

    let transcript = fixture
        .engine
        .transcribe(&ramp(1_600), options("hu"))
        .unwrap();
    assert_eq!(sorted_timings(&transcript), [30]);
}

#[test]
fn empty_text_from_the_backend_gives_an_empty_transcript_without_segments() {
    let fixture = fixture();
    for nothing in ["", "   ", "\n\t "] {
        fixture.backend.answer_next(nothing);
        let transcript = fixture
            .engine
            .transcribe(&ramp(16_000), options("es"))
            .unwrap();
        assert_eq!(transcript.raw_text, "", "{nothing:?}");
        assert!(transcript.segments.is_empty(), "{nothing:?}");
        assert!(!transcript.is_partial);
        assert_eq!(transcript.audio_duration_ms, 1_000);
        assert_eq!(transcript.languages, options("es").languages);
    }
    assert_eq!(fixture.backend.call_count(), 3);
}

#[test]
fn empty_audio_gives_an_empty_transcript_without_touching_the_model() {
    let fixture = fixture();
    let transcript = fixture.engine.transcribe(&[], options("ka+zu")).unwrap();
    assert_eq!(transcript.raw_text, "");
    assert!(transcript.segments.is_empty());
    assert_eq!(transcript.audio_duration_ms, 0);
    assert_eq!(transcript.languages, options("ka+zu").languages);
    assert!(!transcript.is_partial);

    assert_eq!(fixture.backend.load_count(), 0);
    assert_eq!(fixture.backend.call_count(), 0);
    assert!(!fixture.engine.is_loaded());
    assert!(!fixture.store_says_loaded());
}

#[test]
fn the_backend_receives_exactly_the_samples_it_was_given() {
    let fixture = fixture();
    let samples = ramp(12_345);
    fixture.engine.transcribe(&samples, options("am")).unwrap();
    let calls = fixture.backend.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].samples.len(), 12_345);
    assert_eq!(calls[0].samples, samples);
}

#[test]
fn the_backend_is_told_the_models_sample_rate() {
    for rate in [8_000u32, 16_000, 22_050, 48_000] {
        let fixture = Fixture::installed(model_at(rate));
        fixture
            .engine
            .transcribe(&ramp(500), options("hu"))
            .unwrap();
        assert_eq!(fixture.backend.calls()[0].sample_rate_hz, rate);
    }
}

#[test]
fn the_backend_receives_the_options_of_the_request() {
    let fixture = fixture();
    let mut opts = options("zu+es+ka");
    opts.model_id = Some(fixture.model.id.clone());
    fixture.engine.transcribe(&ramp(500), opts.clone()).unwrap();
    assert_eq!(fixture.backend.calls()[0].opts, opts);
}

#[test]
fn every_transcript_has_its_own_id() {
    let fixture = fixture();
    let mut ids = HashSet::new();
    for round in 0..30 {
        // The clock stands still for most rounds: ids must not depend on time passing.
        if round % 10 == 0 {
            fixture.clock.advance_ms(1);
        }
        let transcript = match round % 3 {
            0 => fixture.engine.transcribe(&ramp(800), options("hu")),
            1 => fixture.engine.transcribe(&[], options("hu")),
            _ => {
                fixture.backend.answer_next("");
                fixture.engine.transcribe(&ramp(800), options("hu"))
            }
        }
        .unwrap();
        assert!(!transcript.id.is_empty());
        assert!(
            ids.insert(transcript.id.clone()),
            "the id {} was given out twice",
            transcript.id
        );
    }
}

#[test]
fn transcripts_made_through_copies_of_the_engine_have_different_ids() {
    let fixture = fixture();
    let copy = fixture.engine.clone();
    let first = fixture
        .engine
        .transcribe(&ramp(800), options("hu"))
        .unwrap();
    let second = copy.transcribe(&ramp(800), options("hu")).unwrap();
    assert_ne!(first.id, second.id);
    assert_eq!(fixture.backend.load_count(), 1);
}
