mod support;

use dictation_api::AsrEngine;
use dictation_types::{AsrOptions, ErrorKind, ModelDescriptor, ModelId, Term, TermId, TermKind};
use support::{ENGINE, Fixture, language, options, ramp, speech_entry, with_setting};

const LISTED: [&str; 6] = ["sr", "kk", "te", "no", "pt", "ha"];

fn model(mixing: &str) -> ModelDescriptor {
    speech_entry(
        "model-a",
        ENGINE,
        &LISTED,
        &[("sampleRateHz", "8000"), ("languageMixing", mixing)],
    )
}

/// Asserts the refusal kind and that the backend was never touched.
fn assert_refused(fixture: &Fixture, opts: AsrOptions, kind: ErrorKind) {
    let err = fixture
        .engine
        .start(opts.clone())
        .err()
        .unwrap_or_else(|| panic!("a session was started for {opts:?}"));
    assert_eq!(err.kind, kind, "{err}");
    let err = fixture
        .engine
        .transcribe(&ramp(800), opts.clone())
        .err()
        .unwrap_or_else(|| panic!("audio was recognised for {opts:?}"));
    assert_eq!(err.kind, kind, "{err}");
    assert_eq!(fixture.backend.load_count(), 0);
    assert_eq!(fixture.backend.call_count(), 0);
}

fn assert_accepted(fixture: &Fixture, opts: AsrOptions) {
    fixture
        .engine
        .start(opts.clone())
        .unwrap_or_else(|err| panic!("no session for {opts:?}: {err}"));
    let before = fixture.backend.call_count();
    fixture
        .engine
        .transcribe(&ramp(800), opts.clone())
        .unwrap_or_else(|err| panic!("no recognition for {opts:?}: {err}"));
    assert_eq!(fixture.backend.call_count(), before + 1);
}

#[test]
fn streaming_is_refused() {
    for mixing in ["one", "auto-detect-one", "mixed"] {
        let fixture = Fixture::installed(model(mixing));
        let mut opts = options("kk");
        opts.streaming = true;
        assert_refused(&fixture, opts, ErrorKind::Unsupported);
    }
}

#[test]
fn hotwords_are_refused() {
    let fixture = Fixture::installed(model("mixed"));
    let mut opts = options("te");
    opts.hotwords = vec![
        Term::new(
            TermId::new("term-1"),
            "a rare word",
            TermKind::Word,
            language("te"),
        )
        .unwrap(),
    ];
    assert_refused(&fixture, opts, ErrorKind::Unsupported);
}

#[test]
fn a_language_for_each_segment_is_refused() {
    for mixing in ["one", "auto-detect-one", "mixed"] {
        let fixture = Fixture::installed(model(mixing));
        let mut opts = options("no");
        opts.per_segment_language = true;
        assert_refused(&fixture, opts, ErrorKind::Unsupported);
    }
}

#[test]
fn a_language_the_model_does_not_list_is_refused() {
    for mixing in ["one", "auto-detect-one", "mixed"] {
        let fixture = Fixture::installed(model(mixing));
        for spec in ["is", "gu", "sq"] {
            assert_refused(&fixture, options(spec), ErrorKind::Unsupported);
        }
    }
    for mixing in ["auto-detect-one", "mixed"] {
        let fixture = Fixture::installed(model(mixing));
        for spec in ["sr+is", "gu+kk+te", "pt+ha+sq"] {
            assert_refused(&fixture, options(spec), ErrorKind::Unsupported);
        }
    }
}

#[test]
fn every_listed_language_is_accepted_on_its_own() {
    for mixing in ["one", "auto-detect-one", "mixed"] {
        let fixture = Fixture::installed(model(mixing));
        for tag in LISTED {
            assert_accepted(&fixture, options(tag));
        }
    }
}

#[test]
fn a_regional_variant_of_a_listed_language_is_accepted() {
    let fixture = Fixture::installed(model("one"));
    for tag in ["pt-BR", "pt-PT", "sr-Latn", "sr-Cyrl-RS"] {
        assert_accepted(&fixture, options(tag));
    }
}

#[test]
fn several_languages_are_refused_by_a_model_that_takes_one() {
    let fixture = Fixture::installed(model("one"));
    for spec in ["sr+kk", "te+no+pt", "ha+pt+no+te+kk+sr"] {
        assert_refused(&fixture, options(spec), ErrorKind::Unsupported);
    }
}

#[test]
fn several_listed_languages_are_accepted_by_a_model_that_mixes() {
    let fixture = Fixture::installed(model("mixed"));
    for spec in ["sr+kk", "te+no+pt", "ha+pt+no+te+kk+sr"] {
        assert_accepted(&fixture, options(spec));
    }
}

#[test]
fn several_listed_languages_are_accepted_by_a_model_that_finds_the_one_spoken() {
    let fixture = Fixture::installed(model("auto-detect-one"));
    for spec in ["kk+sr", "pt+te+ha", "sr+kk+te+no+pt+ha"] {
        assert_accepted(&fixture, options(spec));
    }
}

#[test]
fn more_languages_than_the_model_can_mix_are_refused() {
    let limited = with_setting(model("mixed"), "maxMixedLanguages", Some("3"));

    let fixture = Fixture::installed(limited.clone());
    for spec in ["sr+kk+te+no", "ha+pt+no+te+kk", "ha+pt+no+te+kk+sr"] {
        assert_refused(&fixture, options(spec), ErrorKind::Unsupported);
    }

    let fixture = Fixture::installed(limited);
    for spec in ["ha", "no+pt", "sr+kk+te", "te+ha+no"] {
        assert_accepted(&fixture, options(spec));
    }
}

#[test]
fn naming_another_model_is_refused() {
    let fixture = Fixture::installed(model("mixed"));
    for other in ["model-b", "model-a2", "model"] {
        let mut opts = options("sr");
        opts.model_id = Some(ModelId::new(other));
        assert_refused(&fixture, opts, ErrorKind::Validation);
    }
}

#[test]
fn naming_this_model_or_none_is_accepted() {
    let fixture = Fixture::installed(model("mixed"));
    let mut named = options("sr");
    named.model_id = Some(fixture.model.id.clone());
    assert_accepted(&fixture, named);

    let unnamed = options("sr");
    assert_eq!(unnamed.model_id, None);
    assert_accepted(&fixture, unnamed);
}

fn limited_to_two_seconds() -> Fixture {
    Fixture::installed(with_setting(model("mixed"), "maxAudioSecs", Some("2")))
}

#[test]
fn a_recording_longer_than_the_model_takes_is_refused() {
    let fixture = limited_to_two_seconds();
    for samples in [16_001, 16_800, 24_000, 80_000] {
        let err = fixture
            .engine
            .transcribe(&ramp(samples), options("ha"))
            .err()
            .unwrap_or_else(|| panic!("{samples} samples at 8000 Hz were accepted"));
        assert_eq!(err.kind, ErrorKind::Validation, "{err}");
    }
    assert_eq!(fixture.backend.call_count(), 0);
}

#[test]
fn a_recording_of_exactly_the_longest_length_is_accepted() {
    let fixture = limited_to_two_seconds();
    for samples in [1, 8_000, 15_999, 16_000] {
        let transcript = fixture
            .engine
            .transcribe(&ramp(samples), options("ha"))
            .unwrap_or_else(|err| panic!("{samples} samples were refused: {err}"));
        assert!(transcript.audio_duration_ms <= 2_000);
    }
    assert_eq!(fixture.backend.call_count(), 4);
    assert_eq!(fixture.backend.calls()[3].samples.len(), 16_000);
}

#[test]
fn a_session_refuses_audio_as_soon_as_it_is_longer_than_the_model_takes() {
    let fixture = limited_to_two_seconds();
    let mut session = fixture.engine.start(options("no")).unwrap();
    session.push_audio(&ramp(10_000)).unwrap();
    session.push_audio(&ramp(6_000)).unwrap();
    let err = session
        .push_audio(&ramp(1))
        .expect_err("one sample over the limit was accepted");
    assert_eq!(err.kind, ErrorKind::Validation, "{err}");
    assert_eq!(fixture.backend.call_count(), 0);

    let mut session = fixture.engine.start(options("no")).unwrap();
    let err = session
        .push_audio(&ramp(16_001))
        .expect_err("a piece longer than the limit was accepted");
    assert_eq!(err.kind, ErrorKind::Validation, "{err}");
    assert_eq!(fixture.backend.call_count(), 0);
}

#[test]
fn a_session_accepts_audio_of_exactly_the_longest_length() {
    let fixture = limited_to_two_seconds();
    let mut session = fixture.engine.start(options("no")).unwrap();
    for _ in 0..16 {
        session.push_audio(&ramp(1_000)).unwrap();
    }
    let transcript = session.finish().unwrap();
    assert_eq!(transcript.audio_duration_ms, 2_000);
    assert_eq!(fixture.backend.calls()[0].samples.len(), 16_000);
}

#[test]
fn a_model_without_a_longest_length_takes_long_recordings() {
    let fixture = Fixture::installed(model("mixed"));
    let transcript = fixture
        .engine
        .transcribe(&ramp(8_000 * 120), options("te"))
        .unwrap();
    assert_eq!(transcript.audio_duration_ms, 120_000);
}

#[test]
fn a_session_that_refused_audio_keeps_what_it_had_and_can_still_finish() {
    let fixture = limited_to_two_seconds();
    let whole = ramp(16_000);
    let mut session = fixture.engine.start(options("no")).unwrap();
    session.push_audio(&whole[..15_000]).unwrap();
    assert!(session.push_audio(&ramp(1_001)).is_err());
    session.push_audio(&whole[15_000..]).unwrap();
    session.finish().unwrap();
    assert_eq!(fixture.backend.calls()[0].samples, whole);
}
