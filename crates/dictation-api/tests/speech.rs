mod support;

use dictation_api::AsrEngine;
use dictation_types::{AsrOptions, ErrorKind};
use support::{FailAt, Log, LoggingEngine, langs};

fn entries(log: &Log) -> Vec<String> {
    log.lock().unwrap().clone()
}

#[test]
fn one_shot_transcription_starts_a_session_pushes_the_audio_and_finishes() {
    let engine = LoggingEngine::new(FailAt::Nowhere);
    let samples = vec![0.25_f32; 1_600];
    let transcript = engine
        .transcribe(&samples, AsrOptions::new(langs("pt+it")))
        .unwrap();

    assert_eq!(entries(&engine.log), ["start:pt+it", "push:1600", "finish"]);
    assert_eq!(transcript.raw_text, "bom dia, buongiorno");
    assert_eq!(transcript.languages, langs("pt+it"));
}

#[test]
fn one_shot_transcription_passes_the_callers_options_to_the_session() {
    for spec in ["bn+en", "ru+ko+fr", "sw"] {
        let engine = LoggingEngine::new(FailAt::Nowhere);
        let transcript = engine
            .transcribe(&[0.0; 8], AsrOptions::new(langs(spec)))
            .unwrap();
        assert_eq!(entries(&engine.log)[0], format!("start:{spec}"));
        assert_eq!(transcript.languages, langs(spec));
    }
}

#[test]
fn one_shot_transcription_fails_when_the_session_cannot_start() {
    let engine = LoggingEngine::new(FailAt::Start);
    let err = engine
        .transcribe(&[0.0; 8], AsrOptions::new(langs("pt")))
        .unwrap_err();
    assert_eq!(err.kind, ErrorKind::AsrEngine);
    assert_eq!(entries(&engine.log), ["start:pt"]);
}

#[test]
fn one_shot_transcription_fails_when_the_audio_is_refused() {
    let engine = LoggingEngine::new(FailAt::Push);
    let err = engine
        .transcribe(&[0.0; 8], AsrOptions::new(langs("pt")))
        .unwrap_err();
    assert_eq!(err.message, "push failed");
    assert!(!entries(&engine.log).contains(&"finish".to_owned()));
}

#[test]
fn one_shot_transcription_fails_when_finishing_fails() {
    let engine = LoggingEngine::new(FailAt::Finish);
    let err = engine
        .transcribe(&[0.0; 8], AsrOptions::new(langs("pt")))
        .unwrap_err();
    assert_eq!(err.kind, ErrorKind::Timeout);
    assert_eq!(entries(&engine.log), ["start:pt", "push:8", "finish"]);
}

#[test]
fn a_session_has_no_partial_transcript_and_cancels_cleanly_by_default() {
    let engine = LoggingEngine::new(FailAt::Nowhere);
    let mut session = engine.start(AsrOptions::new(langs("it"))).unwrap();
    assert_eq!(session.partial_transcript().unwrap(), None);
    assert!(session.cancel().is_ok());
}
