mod support;

use dictation_api::AsrEngine;
use dictation_types::{AppError, ErrorKind, ModelDescriptor};
use support::{ENGINE, Fixture, options, ramp, speech_entry, tuning};

fn model() -> ModelDescriptor {
    speech_entry(
        "model-a",
        ENGINE,
        &["lt", "ur", "mn", "ca"],
        &[("sampleRateHz", "16000"), ("languageMixing", "mixed")],
    )
}

fn recognise(fixture: &Fixture) {
    fixture
        .engine
        .transcribe(&ramp(1_600), options("ur"))
        .expect("recognised");
}

#[test]
fn the_model_is_loaded_on_first_use_and_only_then() {
    let fixture = Fixture::installed(model());
    assert_eq!(fixture.backend.load_count(), 0);
    for _ in 0..5 {
        recognise(&fixture);
        assert_eq!(fixture.backend.load_count(), 1);
        assert!(fixture.engine.is_loaded());
    }
    assert_eq!(fixture.backend.call_count(), 5);
    assert_eq!(fixture.backend.models_alive(), 1);
}

#[test]
fn the_model_is_loaded_from_the_folder_the_store_reports_with_the_given_tuning() {
    let fixture = Fixture::installed(model());
    recognise(&fixture);
    let loads = fixture.backend.loads();
    assert_eq!(loads.len(), 1);
    assert_eq!(loads[0].dir, fixture.folder());
    assert_eq!(loads[0].model_id, fixture.model.id);
    assert_eq!(loads[0].tuning, tuning());
}

#[test]
fn the_model_can_be_loaded_ahead_of_time() {
    let fixture = Fixture::installed(model());
    fixture.engine.load().unwrap();
    assert!(fixture.engine.is_loaded());
    assert_eq!(fixture.backend.load_count(), 1);
    assert_eq!(fixture.backend.loads()[0].dir, fixture.folder());
    assert_eq!(fixture.backend.call_count(), 0);

    fixture.engine.load().unwrap();
    recognise(&fixture);
    assert_eq!(fixture.backend.load_count(), 1);
}

#[test]
fn a_model_that_is_not_installed_cannot_be_used_and_nothing_is_loaded() {
    let fixture = Fixture::not_installed(model());

    let err = fixture
        .engine
        .transcribe(&ramp(1_600), options("ur"))
        .expect_err("not installed");
    assert_eq!(err.kind, ErrorKind::ModelStore, "{err}");
    let err = fixture.engine.load().expect_err("not installed");
    assert_eq!(err.kind, ErrorKind::ModelStore, "{err}");
    let mut session = fixture.engine.start(options("ur")).unwrap();
    session.push_audio(&ramp(1_600)).unwrap();
    let err = session.finish().expect_err("not installed");
    assert_eq!(err.kind, ErrorKind::ModelStore, "{err}");

    assert_eq!(fixture.backend.load_count(), 0);
    assert_eq!(fixture.backend.call_count(), 0);
    assert_eq!(fixture.backend.models_alive(), 0);
    assert!(!fixture.engine.is_loaded());
}

#[test]
fn a_model_installed_later_is_picked_up() {
    let fixture = Fixture::not_installed(model());
    assert!(fixture.engine.load().is_err());
    fixture.install();
    recognise(&fixture);
    assert_eq!(fixture.backend.load_count(), 1);
    assert_eq!(fixture.backend.loads()[0].dir, fixture.folder());
}

#[test]
fn a_failed_load_is_reported_and_tried_again_next_time() {
    let fixture = Fixture::installed(model());
    fixture
        .backend
        .fail_next_load(AppError::model("the weights are damaged"));

    let err = fixture
        .engine
        .transcribe(&ramp(1_600), options("ur"))
        .expect_err("the load failed");
    assert_eq!(err.kind, ErrorKind::ModelStore);
    assert!(err.message.contains("the weights are damaged"), "{err}");
    assert!(!fixture.engine.is_loaded());
    assert!(!fixture.store_says_loaded());
    assert_eq!(fixture.backend.models_alive(), 0);
    assert_eq!(fixture.backend.call_count(), 0);

    recognise(&fixture);
    assert_eq!(fixture.backend.load_count(), 2);
    assert!(fixture.engine.is_loaded());
    assert!(fixture.store_says_loaded());
}

#[test]
fn a_failed_load_ahead_of_time_is_reported_with_its_kind() {
    let fixture = Fixture::installed(model());
    for error in [
        AppError::storage("the folder cannot be read"),
        AppError::unsupported("this processor cannot run the model"),
    ] {
        fixture.backend.fail_next_load(error.clone());
        let err = fixture.engine.load().expect_err("the load failed");
        assert_eq!(err.kind, error.kind);
        assert!(err.message.contains(&error.message), "{err}");
        assert!(!fixture.engine.is_loaded());
    }
    fixture.engine.load().expect("the third try works");
    assert_eq!(fixture.backend.load_count(), 3);
}

#[test]
fn the_store_is_told_when_the_model_is_loaded_and_when_it_is_let_go() {
    let fixture = Fixture::installed(model());
    assert!(!fixture.store_says_loaded());

    recognise(&fixture);
    assert!(fixture.store_says_loaded());

    assert!(fixture.engine.unload().unwrap());
    assert!(!fixture.store_says_loaded());

    fixture.engine.load().unwrap();
    assert!(fixture.store_says_loaded());
    assert_eq!(fixture.marks(), [true, false, true]);
}

#[test]
fn the_store_is_told_once_for_each_load_not_for_each_recognition() {
    let fixture = Fixture::installed(model());
    for _ in 0..6 {
        recognise(&fixture);
    }
    fixture.engine.load().unwrap();
    assert_eq!(fixture.marks(), [true]);

    assert!(fixture.engine.unload().unwrap());
    assert_eq!(fixture.marks(), [true, false]);

    for _ in 0..3 {
        recognise(&fixture);
    }
    assert_eq!(fixture.marks(), [true, false, true]);
}

#[test]
fn recognising_nothing_tells_the_store_nothing() {
    let fixture = Fixture::installed(model());
    fixture.engine.transcribe(&[], options("ur")).unwrap();
    assert!(fixture.marks().is_empty());
}

#[test]
fn a_load_the_store_cannot_be_told_about_fails_and_leaves_nothing_in_memory() {
    let fixture = Fixture::installed(model());
    fixture.refuse_next_mark_loaded(AppError::storage("the models folder is read-only"));

    let err = fixture
        .engine
        .transcribe(&ramp(1_600), options("ur"))
        .expect_err("the store refused");
    assert_eq!(err.kind, ErrorKind::Storage);
    assert!(err.message.contains("read-only"), "{err}");
    assert!(!fixture.engine.is_loaded());
    assert_eq!(fixture.backend.models_alive(), 0);
    assert_eq!(fixture.backend.call_count(), 0);
    assert!(!fixture.store_says_loaded());

    recognise(&fixture);
    assert_eq!(fixture.backend.load_count(), 2);
    assert_eq!(fixture.backend.models_alive(), 1);
    assert!(fixture.engine.is_loaded());
    assert!(fixture.store_says_loaded());
}

#[test]
fn a_load_ahead_of_time_the_store_cannot_be_told_about_fails_too() {
    let fixture = Fixture::installed(model());
    fixture.refuse_next_mark_loaded(AppError::model("the model was just deleted"));
    let err = fixture.engine.load().expect_err("the store refused");
    assert_eq!(err.kind, ErrorKind::ModelStore);
    assert!(!fixture.engine.is_loaded());
    assert_eq!(fixture.backend.models_alive(), 0);

    fixture.engine.load().expect("the second try works");
    assert_eq!(fixture.backend.load_count(), 2);
    assert_eq!(fixture.backend.models_alive(), 1);
}

#[test]
fn unloading_frees_the_model_and_the_next_use_loads_it_again() {
    let fixture = Fixture::installed(model());
    recognise(&fixture);
    assert_eq!(fixture.backend.models_alive(), 1);

    assert!(fixture.engine.unload().unwrap(), "it was loaded");
    assert_eq!(fixture.backend.models_alive(), 0);
    assert!(!fixture.engine.is_loaded());

    recognise(&fixture);
    assert_eq!(fixture.backend.models_alive(), 1);
    assert_eq!(fixture.backend.load_count(), 2);
}

#[test]
fn unloading_when_nothing_is_loaded_is_fine_and_says_so() {
    let installed = Fixture::installed(model());
    assert!(!installed.engine.unload().expect("nothing to do"));
    installed.engine.load().unwrap();
    assert!(installed.engine.unload().unwrap());
    assert!(!installed.engine.unload().expect("nothing to do"));
    assert_eq!(installed.backend.models_alive(), 0);

    let not_installed = Fixture::not_installed(model());
    assert!(!not_installed.engine.unload().expect("nothing to do"));
}

#[test]
fn an_idle_model_is_unloaded_only_once_the_timeout_has_passed() {
    let fixture = Fixture::installed(model());
    recognise(&fixture);

    fixture.clock.advance_ms(4_999);
    assert!(!fixture.engine.unload_idle(5_000).unwrap());
    assert!(fixture.engine.is_loaded());
    assert_eq!(fixture.backend.models_alive(), 1);
    assert!(fixture.store_says_loaded());

    fixture.clock.advance_ms(1);
    assert!(fixture.engine.unload_idle(5_000).unwrap());
    assert!(!fixture.engine.is_loaded());
    assert_eq!(fixture.backend.models_alive(), 0);
    assert!(!fixture.store_says_loaded());
}

#[test]
fn a_model_loaded_ahead_of_time_counts_as_used_when_it_was_loaded() {
    let fixture = Fixture::installed(model());
    fixture.engine.load().unwrap();
    fixture.clock.advance_ms(299);
    assert!(!fixture.engine.unload_idle(300).unwrap());
    fixture.clock.advance_ms(1);
    assert!(fixture.engine.unload_idle(300).unwrap());
}

#[test]
fn using_the_model_starts_the_idle_time_over() {
    let fixture = Fixture::installed(model());
    recognise(&fixture);

    for _ in 0..4 {
        fixture.clock.advance_ms(700);
        recognise(&fixture);
    }
    fixture.clock.advance_ms(700);
    assert!(
        !fixture.engine.unload_idle(1_000).unwrap(),
        "used 700 ms ago, not idle for 1000 ms"
    );
    fixture.clock.advance_ms(299);
    assert!(!fixture.engine.unload_idle(1_000).unwrap());
    fixture.clock.advance_ms(1);
    assert!(fixture.engine.unload_idle(1_000).unwrap());
    assert_eq!(fixture.backend.load_count(), 1);
}

#[test]
fn nothing_is_unloaded_when_nothing_is_loaded() {
    let fixture = Fixture::installed(model());
    assert!(!fixture.engine.unload_idle(0).unwrap());
    fixture.clock.advance_ms(60_000);
    assert!(!fixture.engine.unload_idle(1).unwrap());

    recognise(&fixture);
    fixture.clock.advance_ms(10);
    assert!(fixture.engine.unload_idle(10).unwrap());
    assert!(!fixture.engine.unload_idle(10).unwrap(), "already unloaded");
}

#[test]
fn a_model_unloaded_for_being_idle_loads_again_on_next_use() {
    let fixture = Fixture::installed(model());
    recognise(&fixture);
    fixture.clock.advance_ms(2_000);
    assert!(fixture.engine.unload_idle(2_000).unwrap());

    fixture.backend.answer_next("back again");
    let transcript = fixture
        .engine
        .transcribe(&ramp(1_600), options("mn"))
        .unwrap();
    assert_eq!(transcript.raw_text, "back again");
    assert_eq!(fixture.backend.load_count(), 2);
    assert_eq!(fixture.backend.models_alive(), 1);
    assert!(fixture.store_says_loaded());

    fixture.clock.advance_ms(1_999);
    assert!(!fixture.engine.unload_idle(2_000).unwrap());
}
