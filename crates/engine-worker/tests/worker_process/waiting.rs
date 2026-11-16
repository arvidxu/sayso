use std::sync::Arc;
use std::thread;
use std::time::Duration;

use asr_local::{ModelBackend, SpeechModel};
use dictation_types::{CancelToken, ErrorKind};

use super::{
    HANG, SLOW, entry, field, folder, options, sound, starts, tuning, wait_for_start,
    worker_backend,
};

const RATE_HZ: u32 = 16_000;
/// Long enough for a thread to start waiting for the one ahead of it. Should it not have yet,
/// the stop reaches it before it starts and it is reported stopped all the same.
const SETTLE: Duration = Duration::from_millis(200);

pub fn a_recognition_stopped_while_waiting_for_another_leaves_the_worker_alone() {
    let model = entry("model-a", "ok");
    let loaded: Arc<dyn SpeechModel> = Arc::from(
        worker_backend()
            .load(&model, &folder(&model), &tuning(), &CancelToken::new())
            .expect("loaded"),
    );
    let begun = starts();
    let first = {
        let loaded = Arc::clone(&loaded);
        thread::spawn(move || {
            loaded.recognise(
                &sound(SLOW + 0.25),
                RATE_HZ,
                &options("is"),
                &CancelToken::new(),
            )
        })
    };
    wait_for_start(begun);
    let stop = CancelToken::new();
    let waiting = {
        let (loaded, stop) = (Arc::clone(&loaded), stop.clone());
        thread::spawn(move || loaded.recognise(&sound(0.0), RATE_HZ, &options("is"), &stop))
    };
    thread::sleep(SETTLE);
    stop.cancel();

    let finished = first
        .join()
        .unwrap()
        .expect("the first recognition finishes");
    let err = waiting
        .join()
        .unwrap()
        .expect_err("the waiting one was stopped");
    assert_eq!(err.kind, ErrorKind::Cancelled, "{err}");
    assert!(
        loaded.is_usable(),
        "the worker was ended for a recognition it never got"
    );
    let after = loaded
        .recognise(&sound(0.0), RATE_HZ, &options("is"), &CancelToken::new())
        .expect("the worker still recognises");
    assert_eq!(field(&finished, "pid"), field(&after, "pid"));
}

pub fn a_recognition_stopped_while_waiting_for_one_that_ended_the_worker_says_it_was_stopped() {
    let model = entry("model-a", "ok");
    let loaded: Arc<dyn SpeechModel> = Arc::from(
        worker_backend()
            .load(&model, &folder(&model), &tuning(), &CancelToken::new())
            .expect("loaded"),
    );
    let stop_first = CancelToken::new();
    let begun = starts();
    let first = {
        let (loaded, stop) = (Arc::clone(&loaded), stop_first.clone());
        thread::spawn(move || loaded.recognise(&sound(HANG - 1.0), RATE_HZ, &options("is"), &stop))
    };
    wait_for_start(begun);
    let stop_waiting = CancelToken::new();
    let waiting = {
        let (loaded, stop) = (Arc::clone(&loaded), stop_waiting.clone());
        thread::spawn(move || loaded.recognise(&sound(0.0), RATE_HZ, &options("is"), &stop))
    };
    thread::sleep(SETTLE);
    // Cancel the waiting one first; cancelling the first then ends the worker.
    stop_waiting.cancel();
    stop_first.cancel();

    let first = first.join().unwrap().expect_err("the first was stopped");
    assert_eq!(first.kind, ErrorKind::Cancelled, "{first}");
    let err = waiting
        .join()
        .unwrap()
        .expect_err("the waiting one was stopped");
    assert_eq!(
        err.kind,
        ErrorKind::Cancelled,
        "a stopped recognition is reported as stopped: {err}"
    );
    assert!(!loaded.is_usable(), "the first stop ended the worker");
}
