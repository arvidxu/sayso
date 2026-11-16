mod support;

use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use dictation_api::AsrEngine;
use dictation_types::{AppResult, ErrorKind, ModelDescriptor, Transcript};
use support::{ENGINE, Fixture, PATIENCE, options, promptly, ramp, speech_entry, wait_until};

fn model() -> ModelDescriptor {
    speech_entry(
        "model-a",
        ENGINE,
        &["sk", "gl", "my", "yo"],
        &[("sampleRateHz", "16000"), ("languageMixing", "mixed")],
    )
}

/// Returns once the backend is busy with the recognition.
fn recognition_in_progress(fixture: &Fixture) -> JoinHandle<AppResult<Transcript>> {
    fixture.backend.hold_recognitions();
    let held_before = fixture.backend.recognitions_held();
    let engine = fixture.engine.clone();
    let worker = thread::spawn(move || engine.transcribe(&ramp(1_600), options("gl")));
    wait_until("the recognition reaches the backend", || {
        fixture.backend.recognitions_held() > held_before
    });
    worker
}

/// Returns once the backend is busy with the load.
fn load_in_progress(fixture: &Fixture) -> JoinHandle<AppResult<()>> {
    fixture.backend.hold_loads();
    let engine = fixture.engine.clone();
    let worker = thread::spawn(move || engine.load());
    wait_until("the load reaches the backend", || {
        fixture.backend.loads_held() == 1
    });
    worker
}

fn finish<T: Send + 'static>(worker: JoinHandle<AppResult<T>>) -> T {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(worker.join());
    });
    receiver
        .recv_timeout(PATIENCE)
        .expect("the thread finished")
        .expect("the thread did not panic")
        .expect("the work succeeded")
}

#[test]
fn unloading_is_refused_while_a_recognition_is_running() {
    let fixture = Fixture::installed(model());
    let worker = recognition_in_progress(&fixture);

    let engine = fixture.engine.clone();
    let err = promptly("unloading", move || engine.unload())
        .expect_err("the model was unloaded while recognising");
    assert_eq!(err.kind, ErrorKind::AlreadyExists, "{err}");
    assert!(fixture.engine.is_loaded());
    assert_eq!(fixture.backend.models_alive(), 1);
    assert!(fixture.store_says_loaded());
    assert_eq!(fixture.marks(), [true]);

    fixture.backend.release_recognitions();
    let transcript = finish(worker);
    assert_eq!(transcript.raw_text, "recognised words");

    assert!(
        fixture
            .engine
            .unload()
            .expect("nothing is running any more")
    );
    assert_eq!(fixture.backend.models_alive(), 0);
    assert_eq!(fixture.marks(), [true, false]);
}

#[test]
fn unloading_is_refused_while_any_of_several_recognitions_is_running() {
    let fixture = Fixture::installed(model());
    let first = recognition_in_progress(&fixture);
    let second = recognition_in_progress(&fixture);
    assert_eq!(fixture.backend.recognitions_held(), 2);

    assert_eq!(
        fixture.engine.unload().unwrap_err().kind,
        ErrorKind::AlreadyExists
    );
    fixture.backend.release_recognitions();
    finish(first);
    finish(second);
    assert!(fixture.engine.unload().unwrap());
    assert_eq!(fixture.backend.load_count(), 1);
}

#[test]
fn a_running_recognition_is_never_idle_however_long_it_takes() {
    let fixture = Fixture::installed(model());
    let worker = recognition_in_progress(&fixture);

    fixture.clock.advance_ms(3_600_000);
    for timeout in [0, 1, 1_000, 3_600_000] {
        let engine = fixture.engine.clone();
        let unloaded = promptly("unloading when idle", move || engine.unload_idle(timeout))
            .expect("asking is not an error");
        assert!(!unloaded, "unloaded while recognising, timeout {timeout}");
    }
    assert!(fixture.engine.is_loaded());
    assert_eq!(fixture.backend.models_alive(), 1);
    assert!(fixture.store_says_loaded());

    fixture.backend.release_recognitions();
    finish(worker);
}

#[test]
fn a_recognition_that_finishes_starts_the_idle_time_over() {
    let fixture = Fixture::installed(model());
    fixture.engine.load().unwrap();
    fixture.clock.advance_ms(900);
    let worker = recognition_in_progress(&fixture);
    fixture.clock.advance_ms(50_000);
    fixture.backend.release_recognitions();
    finish(worker);

    assert!(
        !fixture.engine.unload_idle(1_000).unwrap(),
        "the recognition only just ended"
    );
    fixture.clock.advance_ms(999);
    assert!(!fixture.engine.unload_idle(1_000).unwrap());
    fixture.clock.advance_ms(1);
    assert!(fixture.engine.unload_idle(1_000).unwrap());
    assert_eq!(fixture.backend.models_alive(), 0);
}

#[test]
fn a_recognition_that_fails_lets_the_model_be_unloaded_again() {
    let fixture = Fixture::installed(model());
    fixture
        .backend
        .fail_next_recognition(dictation_types::AppError::asr("the model gave up"));
    assert!(
        fixture
            .engine
            .transcribe(&ramp(1_600), options("my"))
            .is_err()
    );
    assert!(fixture.engine.is_loaded());
    assert!(fixture.engine.unload().expect("nothing is running"));
    assert_eq!(fixture.backend.models_alive(), 0);
}

#[test]
fn asking_whether_the_model_is_loaded_does_not_wait_for_a_load() {
    let fixture = Fixture::installed(model());
    let worker = load_in_progress(&fixture);

    for _ in 0..3 {
        let engine = fixture.engine.clone();
        let loaded = promptly("asking whether the model is loaded", move || {
            engine.is_loaded()
        });
        assert!(!loaded, "the load has not finished");
    }

    fixture.backend.release_loads();
    finish(worker);
    assert!(fixture.engine.is_loaded());
}

#[test]
fn asking_whether_the_model_is_loaded_does_not_wait_for_a_recognition() {
    let fixture = Fixture::installed(model());
    let worker = recognition_in_progress(&fixture);
    let engine = fixture.engine.clone();
    assert!(promptly("asking whether the model is loaded", move || {
        engine.is_loaded()
    }));
    fixture.backend.release_recognitions();
    finish(worker);
}

#[test]
fn unloading_is_refused_while_the_model_is_being_loaded() {
    let fixture = Fixture::installed(model());
    let worker = load_in_progress(&fixture);

    let engine = fixture.engine.clone();
    let result = promptly("unloading", move || engine.unload());
    fixture.backend.release_loads();
    finish(worker);

    let err = result.expect_err("unloading was accepted while the model was being loaded");
    assert_eq!(err.kind, ErrorKind::AlreadyExists, "{err}");
    assert!(fixture.engine.is_loaded());
    assert_eq!(fixture.backend.models_alive(), 1);
    assert!(fixture.store_says_loaded());
}

#[test]
fn recognitions_that_arrive_during_a_load_wait_for_it_instead_of_loading_again() {
    const WAITING: usize = 8;
    let fixture = Fixture::installed(model());
    let loader = load_in_progress(&fixture);

    let (started, has_started) = mpsc::channel();
    let workers: Vec<_> = (0..WAITING)
        .map(|_| {
            let engine = fixture.engine.clone();
            let started = started.clone();
            thread::spawn(move || {
                let _ = started.send(());
                engine.transcribe(&ramp(1_600), options("yo"))
            })
        })
        .collect();
    for _ in 0..WAITING {
        has_started
            .recv_timeout(PATIENCE)
            .expect("the thread started");
    }
    // Give the threads time to get as far as needing the model.
    thread::sleep(std::time::Duration::from_millis(100));
    assert_eq!(fixture.backend.load_count(), 1, "a second load was started");
    assert_eq!(
        fixture.backend.call_count(),
        0,
        "recognised before the load"
    );

    fixture.backend.release_loads();
    finish(loader);
    for worker in workers {
        let transcript = finish(worker);
        assert_eq!(transcript.timings.stages.len(), 1, "it did not load");
    }
    assert_eq!(fixture.backend.load_count(), 1);
    assert_eq!(fixture.backend.models_alive(), 1);
    assert_eq!(fixture.backend.call_count(), WAITING);
    assert_eq!(fixture.marks(), [true]);
}

#[test]
fn recognitions_waiting_for_a_load_that_fails_get_the_model_from_a_new_load() {
    let fixture = Fixture::installed(model());
    fixture
        .backend
        .fail_next_load(dictation_types::AppError::model("the weights are damaged"));
    fixture.backend.hold_loads();
    let engine = fixture.engine.clone();
    let loader = thread::spawn(move || engine.load());
    wait_until("the load reaches the backend", || {
        fixture.backend.loads_held() == 1
    });
    let engine = fixture.engine.clone();
    let waiting = thread::spawn(move || engine.transcribe(&ramp(1_600), options("sk")));
    thread::sleep(std::time::Duration::from_millis(50));

    fixture.backend.release_loads();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(loader.join());
    });
    let failed = receiver
        .recv_timeout(PATIENCE)
        .expect("the load returned")
        .expect("no panic");
    assert_eq!(failed.unwrap_err().kind, ErrorKind::ModelStore);

    finish(waiting);
    assert_eq!(fixture.backend.load_count(), 2);
    assert_eq!(fixture.backend.models_alive(), 1);
}

#[test]
fn stopping_a_recognition_does_not_fail_one_asked_for_afterwards() {
    let fixture = Fixture::installed(model());
    let stopped = recognition_in_progress(&fixture);
    fixture.engine.interrupt();
    let later = recognition_in_progress(&fixture);
    fixture.backend.release_recognitions();

    let err = promptly("the stopped recognition", move || stopped.join().unwrap());
    assert_eq!(err.unwrap_err().kind, ErrorKind::Cancelled);
    assert_eq!(finish(later).raw_text, "recognised words");
    assert_eq!(fixture.backend.load_count(), 2);
}
