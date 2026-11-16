mod support;

use dictation_api::{AsrEngine, AsrSession};
use dictation_types::{AppError, ErrorKind, ModelDescriptor};
use support::{ENGINE, Fixture, options, ramp, speech_entry};

fn model() -> ModelDescriptor {
    speech_entry(
        "model-a",
        ENGINE,
        &["bg", "ml", "et", "af", "ne"],
        &[("sampleRateHz", "16000"), ("languageMixing", "mixed")],
    )
}

fn fixture() -> Fixture {
    Fixture::installed(model())
}

fn session(fixture: &Fixture) -> Box<dyn AsrSession> {
    fixture.engine.start(options("et+ne")).expect("a session")
}

#[test]
fn audio_pushed_in_pieces_is_recognised_as_one_recording_in_order() {
    let fixture = fixture();
    let whole = ramp(10_000);
    let mut session = session(&fixture);
    for piece in [
        &whole[..1],
        &whole[1..3_333],
        &whole[3_333..9_000],
        &whole[9_000..],
    ] {
        session.push_audio(piece).unwrap();
    }
    fixture.backend.answer_next("one recording");
    let transcript = session.finish().unwrap();

    let calls = fixture.backend.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].samples, whole);
    assert_eq!(calls[0].sample_rate_hz, 16_000);
    assert_eq!(calls[0].opts, options("et+ne"));
    assert_eq!(transcript.raw_text, "one recording");
    assert_eq!(transcript.audio_duration_ms, 625);
    assert_eq!(transcript.languages, options("et+ne").languages);
}

#[test]
fn empty_pieces_change_nothing() {
    let fixture = fixture();
    let whole = ramp(4_000);
    let mut session = session(&fixture);
    session.push_audio(&[]).unwrap();
    session.push_audio(&whole[..2_000]).unwrap();
    session.push_audio(&[]).unwrap();
    session.push_audio(&whole[2_000..]).unwrap();
    session.finish().unwrap();
    assert_eq!(fixture.backend.calls()[0].samples, whole);
}

#[test]
fn pushing_audio_gives_no_partial_transcript_and_does_not_reach_the_backend() {
    let fixture = fixture();
    let mut session = session(&fixture);
    for _ in 0..10 {
        assert_eq!(session.push_audio(&ramp(1_600)).unwrap(), None);
        assert_eq!(session.partial_transcript().unwrap(), None);
    }
    assert_eq!(fixture.backend.call_count(), 0);
    assert_eq!(fixture.backend.load_count(), 0);
}

#[test]
fn a_session_without_audio_finishes_with_an_empty_transcript() {
    let fixture = fixture();
    let transcript = session(&fixture).finish().unwrap();
    assert_eq!(transcript.raw_text, "");
    assert!(transcript.segments.is_empty());
    assert_eq!(transcript.audio_duration_ms, 0);
    assert_eq!(fixture.backend.call_count(), 0);
    assert_eq!(fixture.backend.load_count(), 0);
}

#[test]
fn a_finished_session_takes_no_more_audio_and_cannot_finish_again() {
    let fixture = fixture();
    let mut session = session(&fixture);
    session.push_audio(&ramp(1_600)).unwrap();
    session.finish().unwrap();

    assert!(session.push_audio(&ramp(1_600)).is_err());
    assert!(session.push_audio(&[]).is_err());
    assert!(session.finish().is_err());
    assert!(session.finish().is_err());
    assert_eq!(fixture.backend.call_count(), 1);
}

#[test]
fn a_cancelled_session_takes_no_more_audio_and_cannot_finish() {
    let fixture = fixture();
    let mut session = session(&fixture);
    session.push_audio(&ramp(1_600)).unwrap();
    session.cancel().unwrap();

    assert!(session.push_audio(&ramp(1_600)).is_err());
    assert!(session.push_audio(&[]).is_err());
    assert!(session.finish().is_err());
}

#[test]
fn cancelling_never_reaches_the_backend() {
    let fixture = fixture();

    let mut with_audio = session(&fixture);
    with_audio.push_audio(&ramp(8_000)).unwrap();
    with_audio.cancel().unwrap();
    let _ = with_audio.finish();
    let _ = with_audio.push_audio(&ramp(100));
    with_audio.cancel().expect("cancelling twice is fine");

    let mut without_audio = session(&fixture);
    without_audio.cancel().unwrap();

    let mut finished = session(&fixture);
    finished.finish().unwrap();
    finished.cancel().expect("cancelling after the end is fine");

    assert_eq!(fixture.backend.call_count(), 0);
    assert_eq!(fixture.backend.load_count(), 0);
    assert!(!fixture.engine.is_loaded());
}

#[test]
fn a_session_that_failed_to_finish_stays_open_with_its_audio() {
    let fixture = fixture();
    let whole = ramp(6_000);
    let mut session = session(&fixture);
    session.push_audio(&whole[..2_500]).unwrap();
    fixture
        .backend
        .fail_next_recognition(AppError::timeout("recognition took too long"));
    assert_eq!(session.finish().unwrap_err().kind, ErrorKind::Timeout);

    assert_eq!(session.push_audio(&whole[2_500..]).unwrap(), None);
    fixture.backend.answer_next("second try");
    let transcript = session.finish().expect("the cause is gone");
    assert_eq!(transcript.raw_text, "second try");
    assert_eq!(transcript.audio_duration_ms, 375);
    assert_eq!(fixture.backend.calls()[1].samples, whole);
    assert!(session.finish().is_err(), "ended by the successful finish");
    assert!(session.push_audio(&ramp(1)).is_err());
}

#[test]
fn a_session_can_finish_once_its_model_has_been_installed() {
    let fixture = Fixture::not_installed(model());
    let whole = ramp(4_800);
    let mut session = session(&fixture);
    session.push_audio(&whole).unwrap();
    assert_eq!(session.finish().unwrap_err().kind, ErrorKind::ModelStore);

    fixture.install();
    session.finish().expect("the model is there now");
    assert_eq!(fixture.backend.calls()[0].samples, whole);
}

#[test]
fn a_session_that_failed_to_finish_can_be_cancelled() {
    let fixture = fixture();
    let mut cancelled = session(&fixture);
    cancelled.push_audio(&ramp(1_600)).unwrap();
    fixture
        .backend
        .fail_next_recognition(AppError::asr("the model gave up"));
    assert!(cancelled.finish().is_err());
    cancelled.cancel().unwrap();
    assert!(cancelled.finish().is_err(), "ended by the cancel");
    assert!(cancelled.push_audio(&ramp(1)).is_err());
    assert_eq!(fixture.backend.call_count(), 1);
}

#[test]
fn sessions_of_one_engine_do_not_share_audio() {
    let fixture = fixture();
    let first_audio = ramp(3_000);
    let second_audio: Vec<f32> = ramp(5_000).into_iter().map(|s| -s).collect();
    let mut first = session(&fixture);
    let mut second = session(&fixture);
    first.push_audio(&first_audio[..1_000]).unwrap();
    second.push_audio(&second_audio[..4_000]).unwrap();
    first.push_audio(&first_audio[1_000..]).unwrap();
    second.push_audio(&second_audio[4_000..]).unwrap();

    second.finish().unwrap();
    first.finish().unwrap();
    let calls = fixture.backend.calls();
    assert_eq!(calls[0].samples, second_audio);
    assert_eq!(calls[1].samples, first_audio);
    assert_eq!(fixture.backend.load_count(), 1);
}

#[test]
fn an_error_from_recognition_is_passed_on_with_its_kind() {
    let errors = [
        AppError::asr("the model gave up"),
        AppError::internal("out of memory"),
        AppError::timeout("recognition took too long"),
        AppError::unsupported("this audio cannot be decoded"),
    ];
    for error in errors {
        let fixture = fixture();

        fixture.backend.fail_next_recognition(error.clone());
        let mut session = session(&fixture);
        session.push_audio(&ramp(1_600)).unwrap();
        let err = session.finish().expect_err("recognition failed");
        assert_eq!(err.kind, error.kind);
        assert!(err.message.contains(&error.message), "{err}");

        fixture.backend.fail_next_recognition(error.clone());
        let err = fixture
            .engine
            .transcribe(&ramp(1_600), options("af"))
            .expect_err("recognition failed");
        assert_eq!(err.kind, error.kind);
        assert!(err.message.contains(&error.message), "{err}");
    }
}

#[test]
fn recognition_works_again_after_an_error() {
    let fixture = fixture();
    fixture
        .backend
        .fail_next_recognition(AppError::asr("the model gave up"));
    assert!(
        fixture
            .engine
            .transcribe(&ramp(1_600), options("bg"))
            .is_err()
    );

    fixture.backend.answer_next("fine again");
    let transcript = fixture
        .engine
        .transcribe(&ramp(1_600), options("bg"))
        .unwrap();
    assert_eq!(transcript.raw_text, "fine again");
    assert_eq!(fixture.backend.load_count(), 1);
    assert!(matches!(
        fixture.engine.transcribe(&ramp(1_600), options("bg")),
        Ok(t) if t.audio_duration_ms == 100
    ));
}
