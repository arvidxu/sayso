use std::thread::available_parallelism;

use asr_local::RecognitionTuning;
use dictation_types::ErrorKind;

fn tuning(thread_share: f32, execution_provider: &str) -> RecognitionTuning {
    RecognitionTuning {
        thread_share,
        execution_provider: execution_provider.to_owned(),
        load_timeout_ms: 60_000,
        worker_parent_check_ms: 1_000,
    }
}

fn assert_refused(tuning: RecognitionTuning) {
    match tuning.validate() {
        Ok(()) => panic!("{tuning:?} was accepted"),
        Err(err) => assert_eq!(err.kind, ErrorKind::Validation, "{tuning:?}: {err}"),
    }
}

#[test]
fn a_thread_share_of_zero_or_less_is_refused() {
    for share in [0.0, -0.0, -0.01, -1.0, f32::NEG_INFINITY] {
        assert_refused(tuning(share, "test-processor"));
    }
}

#[test]
fn a_thread_share_above_one_is_refused() {
    for share in [1.000_001, 1.5, 2.0, 100.0, f32::INFINITY] {
        assert_refused(tuning(share, "test-processor"));
    }
}

#[test]
fn a_thread_share_that_is_not_a_number_is_refused() {
    assert_refused(tuning(f32::NAN, "test-processor"));
}

#[test]
fn an_empty_execution_provider_is_refused() {
    for blank in ["", " ", "\t\n"] {
        assert_refused(tuning(0.5, blank));
    }
}

#[test]
fn an_execution_provider_with_control_characters_is_refused() {
    for name in [
        "test\0processor",
        "test-processor\0",
        "\0",
        "test\nprocessor",
        "test-processor\n",
        "test\rprocessor",
        "test\u{1b}processor",
        "test\u{7f}processor",
        "test\u{85}processor",
    ] {
        assert_refused(tuning(0.5, name));
    }
}

#[test]
fn shares_above_zero_up_to_one_are_accepted() {
    for share in [f32::MIN_POSITIVE, 0.001, 0.25, 0.5, 0.999, 1.0] {
        tuning(share, "test-processor")
            .validate()
            .unwrap_or_else(|err| panic!("{share} was refused: {err}"));
    }
}

#[test]
fn any_named_execution_provider_is_accepted() {
    for name in ["test-processor", "graphics", "x"] {
        tuning(0.5, name).validate().expect("accepted");
    }
}

#[test]
fn the_thread_count_is_at_least_one_and_at_most_the_logical_processors() {
    let processors = available_parallelism().expect("the machine says").get();
    for share in [f32::MIN_POSITIVE, 0.001, 0.1, 0.25, 0.5, 0.75, 0.999, 1.0] {
        let threads = tuning(share, "test-processor").threads();
        assert!(threads >= 1, "share {share} gives {threads} threads");
        assert!(
            threads <= processors,
            "share {share} gives {threads} threads on {processors} processors"
        );
    }
}

#[test]
fn a_full_share_uses_every_logical_processor() {
    let processors = available_parallelism().expect("the machine says").get();
    assert_eq!(tuning(1.0, "test-processor").threads(), processors);
}

#[test]
fn a_larger_share_never_gives_fewer_threads() {
    let mut previous = 0;
    for step in 1..=100 {
        let threads = tuning(step as f32 / 100.0, "test-processor").threads();
        assert!(threads >= previous, "share {step}% gives fewer threads");
        previous = threads;
    }
}
