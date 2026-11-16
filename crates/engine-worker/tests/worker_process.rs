//! This test re-runs itself as the worker process, so it has its own entry point instead of
//! the usual test harness.

use std::env;
use std::io::{stdin, stdout};
use std::panic::catch_unwind;
use std::path::{Path, PathBuf};
use std::process::{ExitCode, exit};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use asr_local::{LocalAsrEngine, ModelBackend, RecognitionTuning, SpeechModel};
use dictation_api::fakes::{FakeClock, FakeModelStore};
use dictation_api::{AsrEngine, ModelStore};
use dictation_types::{
    AppResult, AsrOptions, CancelToken, ErrorKind, Language, Languages, ModelDescriptor, ModelFile,
    ModelId, ModelKind, PlatformTarget, ProviderId, Term, TermId, TermKind,
};
use engine_worker::{WorkerBackend, WorkerCommand, serve};

const CHILD_FLAG: &str = "ENGINE_WORKER_TEST_CHILD";
/// The file the test engine adds a line to each time a load or recognition begins, so a test
/// can wait for that instead of guessing how long it takes.
const STARTS_FILE: &str = "ENGINE_WORKER_TEST_STARTS";
/// Makes this program act as the main program: start a worker, get it stuck, and wait.
const MAIN_FLAG: &str = "ENGINE_WORKER_TEST_MAIN";
const ENGINE: &str = "engine-for-tests";
const ON_LOAD: &str = "onLoad";
/// A first sample above this kills the worker's process during recognition.
const STOP_PROCESS: f32 = 1.5;
/// A first sample below this makes the recognition never end.
const HANG: f32 = -1.5;
/// A first sample in (SLOW, STOP_PROCESS] makes the recognition take [`SLOW_FOR`].
const SLOW: f32 = 1.0;
const SLOW_FOR: Duration = Duration::from_millis(1_000);
const PATIENCE: Duration = Duration::from_secs(20);

#[path = "worker_process/engine.rs"]
mod engine;
#[path = "worker_process/support.rs"]
mod support;

use engine::TestEngine;
use support::*;

#[path = "worker_process/waiting.rs"]
mod waiting;

/// Prints its worker's id, then gets the worker stuck in a recognition that never ends.
fn act_as_main_program() -> ExitCode {
    let engine = engine_for(&entry("model-a", "ok"));
    let text = engine
        .transcribe(&sound(0.0), options("is"))
        .expect("a first recognition")
        .raw_text;
    println!("{}", field(&text, "pid"));
    std::io::Write::flush(&mut stdout()).ok();
    engine.transcribe(&sound(HANG - 1.0), options("is")).ok();
    ExitCode::FAILURE
}

fn a_stuck_worker_ends_when_the_main_program_dies() {
    use std::io::BufRead;
    let mut main_program = std::process::Command::new(env::current_exe().unwrap())
        .env_remove(CHILD_FLAG)
        .env(MAIN_FLAG, "1")
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("the stand-in main program starts");
    let mut line = String::new();
    std::io::BufReader::new(main_program.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let worker = line.trim().to_owned();
    assert!(is_running(&worker), "no worker '{worker}'");
    // Let the worker get stuck, then kill the main program as a crash would.
    thread::sleep(Duration::from_millis(300));
    main_program.kill().unwrap();
    main_program.wait().unwrap();
    let started = Instant::now();
    while is_running(&worker) {
        assert!(
            started.elapsed() < PATIENCE,
            "the worker outlived the main program"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

fn what_is_recognised_arrives_intact() {
    let model = entry("model-a", "ok");
    let engine = engine_for(&model);
    let samples = sound(0.125);
    let expected: f64 = samples.iter().map(|s| f64::from(*s)).sum();

    let text = engine
        .transcribe(&samples, options("ta+sw"))
        .unwrap()
        .raw_text;

    assert_eq!(field(&text, "n"), "4000", "{text}");
    assert_eq!(field(&text, "rate"), "16000", "{text}");
    assert_eq!(field(&text, "sum"), format!("{expected:.4}"), "{text}");
    assert_eq!(field(&text, "langs"), "ta+sw", "{text}");
    assert_eq!(field(&text, "from"), folder(&model).display().to_string());
    assert_eq!(field(&text, "share"), "0.25", "{text}");
    assert_eq!(field(&text, "on"), "test-processor", "{text}");
    assert_ne!(
        field(&text, "pid"),
        std::process::id().to_string(),
        "{text}"
    );
}

fn words_to_listen_for_reach_the_engine_unless_switched_off() {
    let engine = engine_for(&entry("model-a", "ok"));
    let language = Language::parse("is").unwrap();
    let term = |number: &str, word: &str| {
        Term::new(
            TermId::new(number),
            word,
            TermKind::default(),
            language.clone(),
        )
        .unwrap()
    };
    let mut opts = options("is");
    let mut off = term("2", "Hafnarfjörður");
    off.enabled = false;
    opts.hotwords = vec![term("1", "Þingvellir"), off, term("3", "Eyjafjallajökull")];
    let text = engine.transcribe(&sound(0.0), opts).unwrap().raw_text;
    assert_eq!(field(&text, "hot"), "Þingvellir|Eyjafjallajökull", "{text}");

    let text = engine
        .transcribe(&sound(0.0), options("is"))
        .unwrap()
        .raw_text;
    assert_eq!(field(&text, "hot"), "", "{text}");
}

fn one_worker_serves_recognition_after_recognition() {
    let engine = engine_for(&entry("model-a", "ok"));
    let first = engine
        .transcribe(&sound(0.0), options("is"))
        .unwrap()
        .raw_text;
    let second = engine
        .transcribe(&sound(0.5), options("sw"))
        .unwrap()
        .raw_text;
    assert_eq!(field(&first, "pid"), field(&second, "pid"));
    assert_ne!(field(&first, "sum"), field(&second, "sum"));
}

fn a_load_the_engine_refuses_is_reported_as_the_engine_said_it() {
    let engine = engine_for(&entry("model-a", "fail"));
    let err = engine.load().expect_err("the load fails");
    assert_eq!(err.kind, ErrorKind::ChecksumMismatch, "{err}");
    assert_eq!(err.message, "the weights are damaged");
    assert!(!engine.is_loaded());
}

fn an_engine_that_stops_its_process_while_loading_only_fails_the_load() {
    let model = entry("model-a", "stop-process");
    let engine = engine_for(&model);
    let err = engine.load().expect_err("the load fails");
    assert_eq!(err.kind, ErrorKind::AsrEngine, "{err}");
    assert!(err.message.contains("model-a"), "{err}");
    assert!(!engine.is_loaded());
    // Still failing the same way instead of taking this program down.
    let err = engine
        .transcribe(&sound(0.0), options("is"))
        .expect_err("the load fails again");
    assert_eq!(err.kind, ErrorKind::AsrEngine, "{err}");
}

fn an_engine_that_stops_its_process_while_recognising_only_fails_that_recognition() {
    let engine = engine_for(&entry("model-a", "ok"));
    let before = engine
        .transcribe(&sound(0.0), options("is"))
        .unwrap()
        .raw_text;

    let err = engine
        .transcribe(&sound(STOP_PROCESS + 1.0), options("is"))
        .expect_err("the worker is gone");
    assert_eq!(err.kind, ErrorKind::AsrEngine, "{err}");
    assert!(!engine.is_loaded(), "a dead worker is not a loaded model");

    let after = engine
        .transcribe(&sound(0.0), options("is"))
        .unwrap()
        .raw_text;
    assert!(engine.is_loaded());
    assert_ne!(
        field(&before, "pid"),
        field(&after, "pid"),
        "a fresh worker"
    );
    assert_eq!(field(&before, "sum"), field(&after, "sum"));
}

fn a_recognition_that_hangs_can_be_stopped_and_the_next_one_works() {
    let engine = engine_for(&entry("model-a", "ok"));
    let before = engine
        .transcribe(&sound(0.0), options("is"))
        .unwrap()
        .raw_text;

    let running = engine.clone();
    let begun = starts();
    let recognition = thread::spawn(move || running.transcribe(&sound(HANG - 1.0), options("is")));
    wait_for_start(begun);
    AsrEngine::interrupt(&engine);
    let err = recognition.join().unwrap().expect_err("it was stopped");
    assert_eq!(err.kind, ErrorKind::Cancelled, "{err}");
    assert!(!engine.is_loaded());

    let after = engine
        .transcribe(&sound(0.0), options("is"))
        .unwrap()
        .raw_text;
    assert_ne!(
        field(&before, "pid"),
        field(&after, "pid"),
        "a fresh worker"
    );
}

fn a_load_that_takes_longer_than_allowed_is_given_up() {
    let limit_ms = 300;
    let engine = engine_with(
        &entry("model-a", "hang"),
        RecognitionTuning {
            load_timeout_ms: limit_ms,
            ..tuning()
        },
    );
    let started = Instant::now();
    let err = engine.load().expect_err("it never finishes");
    assert_eq!(err.kind, ErrorKind::Timeout, "{err}");
    assert!(err.message.contains("model-a"), "{err}");
    assert!(started.elapsed() >= Duration::from_millis(limit_ms));
    assert!(started.elapsed() < PATIENCE, "given up far too late");
    assert!(!engine.is_loaded());
}

fn a_load_that_hangs_can_be_stopped() {
    let engine = engine_for(&entry("model-a", "hang"));
    let loading = engine.clone();
    let begun = starts();
    let load = thread::spawn(move || loading.load());
    wait_for_start(begun);
    AsrEngine::interrupt(&engine);
    let err = load.join().unwrap().expect_err("it was stopped");
    assert_eq!(err.kind, ErrorKind::Cancelled, "{err}");
    assert!(!engine.is_loaded());
}

fn a_time_limit_does_not_disturb_a_load_that_finishes_in_time() {
    let engine = engine_with(
        &entry("model-a", "ok"),
        RecognitionTuning {
            load_timeout_ms: 20_000,
            ..tuning()
        },
    );
    let started = Instant::now();
    engine.load().unwrap();
    assert!(
        started.elapsed() < Duration::from_millis(10_000),
        "waited for the limit"
    );
    let text = engine
        .transcribe(&sound(0.0), options("is"))
        .unwrap()
        .raw_text;
    assert_eq!(field(&text, "n"), "4000", "{text}");
}

fn interrupting_when_nothing_runs_leaves_the_worker_alone() {
    let engine = engine_for(&entry("model-a", "ok"));
    let before = engine
        .transcribe(&sound(0.0), options("is"))
        .unwrap()
        .raw_text;
    engine.interrupt();
    assert!(engine.is_loaded());
    let after = engine
        .transcribe(&sound(0.0), options("is"))
        .unwrap()
        .raw_text;
    assert_eq!(field(&before, "pid"), field(&after, "pid"));
}

fn what_an_engine_prints_on_the_reply_stream_is_not_taken_for_a_reply() {
    let engine = engine_for(&entry("model-a", "chatter"));
    let text = engine
        .transcribe(&sound(0.0), options("is"))
        .unwrap()
        .raw_text;
    assert_eq!(field(&text, "n"), "4000", "{text}");
}

fn a_worker_program_that_does_not_exist_fails_the_load() {
    let model = entry("model-a", "ok");
    let backend = WorkerBackend::new(
        Arc::new(TestEngine),
        WorkerCommand::new("no-such-folder/no-such-worker"),
    );
    let err = backend
        .load(&model, &folder(&model), &tuning(), &CancelToken::new())
        .err()
        .expect("nothing to start");
    assert_eq!(err.kind, ErrorKind::AsrEngine, "{err}");
    assert!(err.message.contains("no-such-worker"), "{err}");
}

fn a_worker_program_missing_beside_this_program_is_reported_with_its_place() {
    // Nothing puts a worker program next to this test.
    let err = WorkerCommand::beside_current_program().expect_err("there is none here");
    assert_eq!(err.kind, ErrorKind::NotFound, "{err}");
    let folder = env::current_exe().unwrap();
    let folder = folder.parent().unwrap().display().to_string();
    assert!(err.message.contains(&folder), "{err}");
}

fn a_worker_built_without_the_engine_says_so() {
    let mut model = entry("model-a", "ok");
    model.engine = ProviderId::new("some-other-engine");
    struct OtherEngine;
    impl ModelBackend for OtherEngine {
        fn engine_id(&self) -> ProviderId {
            ProviderId::new("some-other-engine")
        }
        fn check(&self, _: &ModelDescriptor) -> AppResult<()> {
            Ok(())
        }
        fn load(
            &self,
            _: &ModelDescriptor,
            _: &Path,
            _: &RecognitionTuning,
            _: &CancelToken,
        ) -> AppResult<Box<dyn SpeechModel>> {
            unreachable!("never loaded in the main program")
        }
    }
    let me = env::current_exe().unwrap();
    let backend = WorkerBackend::new(Arc::new(OtherEngine), WorkerCommand::new(me));
    let err = backend
        .load(&model, &folder(&model), &tuning(), &CancelToken::new())
        .err()
        .expect("the worker has no such engine");
    assert_eq!(err.kind, ErrorKind::Unsupported, "{err}");
    assert!(err.message.contains("some-other-engine"), "{err}");
}

fn unloading_the_model_ends_its_worker() {
    let engine = engine_for(&entry("model-a", "ok"));
    let text = engine
        .transcribe(&sound(0.0), options("is"))
        .unwrap()
        .raw_text;
    let pid = field(&text, "pid").to_owned();
    assert!(is_running(&pid));
    assert!(engine.unload().unwrap());
    assert!(!is_running(&pid), "the worker outlived its model");
}

fn main() -> ExitCode {
    if env::var_os(CHILD_FLAG).is_some() {
        let engines: Vec<Arc<dyn ModelBackend>> = vec![Arc::new(TestEngine)];
        let output = engine_worker::reply_output().expect("replies can be sent");
        return match serve(&engines, stdin().lock(), output) {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => ExitCode::FAILURE,
        };
    }
    // Workers started from here on are this program again, told to be the worker.
    // SAFETY: nothing else runs yet, so no other thread reads the environment.
    unsafe { env::set_var(CHILD_FLAG, "1") };
    if env::var_os(MAIN_FLAG).is_some() {
        return act_as_main_program();
    }
    let starts = env::temp_dir().join(format!("engine-worker-starts-{}", std::process::id()));
    let _ = std::fs::remove_file(&starts);
    // SAFETY: as above.
    unsafe { env::set_var(STARTS_FILE, &starts) };
    thread::spawn(|| {
        thread::sleep(PATIENCE * 6);
        eprintln!("the tests took too long; something hangs");
        exit(1);
    });

    let tests: &[(&str, fn())] = &[
        (
            "what_is_recognised_arrives_intact",
            what_is_recognised_arrives_intact,
        ),
        (
            "words_to_listen_for_reach_the_engine_unless_switched_off",
            words_to_listen_for_reach_the_engine_unless_switched_off,
        ),
        (
            "one_worker_serves_recognition_after_recognition",
            one_worker_serves_recognition_after_recognition,
        ),
        (
            "a_load_the_engine_refuses_is_reported_as_the_engine_said_it",
            a_load_the_engine_refuses_is_reported_as_the_engine_said_it,
        ),
        (
            "an_engine_that_stops_its_process_while_loading_only_fails_the_load",
            an_engine_that_stops_its_process_while_loading_only_fails_the_load,
        ),
        (
            "an_engine_that_stops_its_process_while_recognising_only_fails_that_recognition",
            an_engine_that_stops_its_process_while_recognising_only_fails_that_recognition,
        ),
        (
            "a_recognition_that_hangs_can_be_stopped_and_the_next_one_works",
            a_recognition_that_hangs_can_be_stopped_and_the_next_one_works,
        ),
        (
            "a_load_that_takes_longer_than_allowed_is_given_up",
            a_load_that_takes_longer_than_allowed_is_given_up,
        ),
        (
            "a_stuck_worker_ends_when_the_main_program_dies",
            a_stuck_worker_ends_when_the_main_program_dies,
        ),
        (
            "a_load_that_hangs_can_be_stopped",
            a_load_that_hangs_can_be_stopped,
        ),
        (
            "a_time_limit_does_not_disturb_a_load_that_finishes_in_time",
            a_time_limit_does_not_disturb_a_load_that_finishes_in_time,
        ),
        (
            "interrupting_when_nothing_runs_leaves_the_worker_alone",
            interrupting_when_nothing_runs_leaves_the_worker_alone,
        ),
        (
            "a_recognition_stopped_while_waiting_for_another_leaves_the_worker_alone",
            waiting::a_recognition_stopped_while_waiting_for_another_leaves_the_worker_alone,
        ),
        (
            "a_recognition_stopped_while_waiting_for_one_that_ended_the_worker_says_it_was_stopped",
            waiting::a_recognition_stopped_while_waiting_for_one_that_ended_the_worker_says_it_was_stopped,
        ),
        (
            "what_an_engine_prints_on_the_reply_stream_is_not_taken_for_a_reply",
            what_an_engine_prints_on_the_reply_stream_is_not_taken_for_a_reply,
        ),
        (
            "a_worker_program_that_does_not_exist_fails_the_load",
            a_worker_program_that_does_not_exist_fails_the_load,
        ),
        (
            "a_worker_program_missing_beside_this_program_is_reported_with_its_place",
            a_worker_program_missing_beside_this_program_is_reported_with_its_place,
        ),
        (
            "a_worker_built_without_the_engine_says_so",
            a_worker_built_without_the_engine_says_so,
        ),
        (
            "unloading_the_model_ends_its_worker",
            unloading_the_model_ends_its_worker,
        ),
    ];
    let mut failed = 0;
    for (name, test) in tests {
        let passed = catch_unwind(test).is_ok();
        println!("test {name} ... {}", if passed { "ok" } else { "FAILED" });
        failed += usize::from(!passed);
    }
    println!("{} passed, {failed} failed", tests.len() - failed);
    let _ = std::fs::remove_file(&starts);
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
