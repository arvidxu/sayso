mod insertion_support;

use std::time::Duration;

use dictation_api::TextInjector;
use dictation_types::{ErrorKind, InjectionWarning, InsertMethod, PasteRead};
use insertion_support::*;
use platform_macos::ClipboardContents;
use platform_macos::insertion::MAX_TYPED_UNITS;

#[test]
fn pasting_saves_the_clipboard_pastes_marked_transient_then_puts_everything_back() {
    let fake = Fake::with(|w| w.contents = image_and_text());
    let outcome = fake
        .inserter()
        .insert("dictated", InsertMethod::PasteWithClipboardRestore)
        .unwrap();
    assert_eq!(
        fake.steps(),
        [
            Step::Read,
            Step::Offer("dictated".to_owned()),
            Step::Paste,
            Step::Restore(image_and_text()),
        ]
    );
    assert_eq!(fake.contents(), image_and_text());
    assert_eq!(outcome.method_used, InsertMethod::PasteWithClipboardRestore);
    assert!(outcome.clipboard_restored);
    assert_eq!(outcome.paste_read, Some(PasteRead::Read));
    assert!(!outcome.fallback_to_clipboard);
    assert_eq!(outcome.diagnostic, None);
}

#[test]
fn a_paste_says_plainly_whether_an_application_asked_for_the_text() {
    let unread = Fake::with(|w| w.nobody_reads = true);
    let outcome = unread
        .inserter_with(tuning(0, 30, 0))
        .insert("dictated", InsertMethod::PasteWithClipboardRestore)
        .unwrap();
    assert_eq!(outcome.paste_read, Some(PasteRead::Unread));
    assert_eq!(outcome.diagnostic, None);

    let taken = Fake::with(|w| w.read_before_keys = true);
    let outcome = taken
        .inserter_with(tuning(0, 30, 0))
        .insert("dictated", InsertMethod::PasteWithClipboardRestore)
        .unwrap();
    assert_eq!(outcome.paste_read, Some(PasteRead::TakenBeforeKeys));
    assert_eq!(outcome.diagnostic, None);
}

#[test]
fn methods_that_do_not_watch_the_clipboard_report_no_read_result() {
    for method in [
        InsertMethod::PasteKeepingText,
        InsertMethod::SimulatedTyping,
        InsertMethod::CopyOnly,
    ] {
        let outcome = Fake::default()
            .inserter()
            .insert("dictated", method)
            .unwrap();
        assert_eq!(outcome.paste_read, None, "{method}");
    }
}

#[test]
fn an_empty_clipboard_is_put_back_empty() {
    let fake = Fake::default();
    fake.inserter()
        .insert("dictated", InsertMethod::PasteWithClipboardRestore)
        .unwrap();
    assert_eq!(fake.contents(), ClipboardContents::default());
}

#[test]
fn a_clipboard_changed_by_something_else_while_pasting_is_left_alone() {
    let fake = Fake::with(|w| {
        w.contents = text_contents("before");
        w.other_writer = true;
    });
    let outcome = fake
        .inserter()
        .insert("dictated", InsertMethod::PasteWithClipboardRestore)
        .unwrap();
    assert_eq!(fake.contents(), text_contents("copied by the user"));
    assert!(!outcome.clipboard_restored);
    assert!(outcome.diagnostic.unwrap().contains("left as it is"));
}

#[test]
fn a_failed_paste_puts_the_clipboard_back_and_fails() {
    let fake = Fake::with(|w| {
        w.contents = text_contents("before");
        w.fail_paste = true;
    });
    let err = fake
        .inserter()
        .insert("dictated", InsertMethod::PasteWithClipboardRestore)
        .unwrap_err();
    assert_eq!(err.kind, ErrorKind::TextInjection);
    assert_eq!(fake.contents(), text_contents("before"));
}

#[test]
fn a_paste_whose_clipboard_cannot_be_put_back_still_succeeds_and_says_so() {
    let fake = Fake::with(|w| w.fail_restore = true);
    let outcome = fake
        .inserter()
        .insert("dictated", InsertMethod::PasteWithClipboardRestore)
        .unwrap();
    assert!(!outcome.clipboard_restored);
    assert!(
        outcome
            .diagnostic
            .unwrap()
            .contains("the clipboard is locked")
    );
}

#[test]
fn typing_sends_the_text_in_runs_and_leaves_the_clipboard_alone() {
    // In the second text, a character of two code units falls across the end of the first run.
    for text in [
        "Grüße, 你好世界！🎙️ مرحبا — this sentence is longer than one typed event.",
        "x😀😀😀😀😀😀😀😀😀😀",
    ] {
        let fake = Fake::with(|w| w.contents = text_contents("before"));
        let outcome = fake
            .inserter()
            .insert(text, InsertMethod::SimulatedTyping)
            .unwrap();
        let steps = fake.steps();
        let typed: String = steps
            .iter()
            .map(|step| match step {
                Step::Type(run) => run.as_str(),
                other => panic!("typing touched the clipboard: {other:?}"),
            })
            .collect();
        assert_eq!(typed, text);
        assert!(steps.len() > 1);
        assert_eq!(outcome.method_used, InsertMethod::SimulatedTyping);
        assert!(!outcome.clipboard_restored);
        assert_eq!(fake.contents(), text_contents("before"));
    }
}

#[test]
fn typing_stops_at_the_first_run_that_fails() {
    let fake = Fake::with(|w| w.fail_typing_after = Some(1));
    let text = "a".repeat(MAX_TYPED_UNITS * 3);
    assert!(
        fake.inserter()
            .insert(&text, InsertMethod::SimulatedTyping)
            .is_err()
    );
    assert_eq!(fake.steps().len(), 1);
}

#[test]
fn secure_input_is_a_warning_only_while_it_is_on_and_text_still_goes_in() {
    let fake = Fake::default();
    assert_eq!(fake.inserter().check_injection_warning(), None);
    for method in InsertMethod::ALL {
        assert_eq!(
            fake.inserter().insert("text", method).unwrap().warning,
            None
        );
    }
    fake.0.lock().unwrap().secure = true;
    assert_eq!(
        fake.inserter().check_injection_warning(),
        Some(InjectionWarning::SecureInputActive)
    );
    for method in [
        InsertMethod::PasteWithClipboardRestore,
        InsertMethod::PasteKeepingText,
        InsertMethod::SimulatedTyping,
    ] {
        let secure = Fake::with(|w| w.secure = true);
        let outcome = secure.inserter().insert("text", method).unwrap();
        assert_eq!(
            outcome.warning,
            Some(InjectionWarning::SecureInputActive),
            "{method}"
        );
        assert!(
            secure
                .steps()
                .iter()
                .any(|step| matches!(step, Step::Paste | Step::Type(_))),
            "{method}"
        );
    }
}

#[test]
fn copying_only_writes_the_text_for_keeps() {
    // Copying needs no permission to send keys.
    let fake = Fake::with(|w| {
        w.contents = text_contents("before");
        w.keys_dropped = true;
    });
    let outcome = fake
        .inserter()
        .insert("dictated", InsertMethod::CopyOnly)
        .unwrap();
    assert_eq!(
        fake.steps(),
        [Step::Write {
            text: "dictated".to_owned(),
            transient: false
        }]
    );
    assert_eq!(outcome.method_used, InsertMethod::CopyOnly);
    assert!(!outcome.clipboard_restored);
}

#[test]
fn pasting_keeping_the_text_writes_it_for_keeps_and_never_puts_the_clipboard_back() {
    let fake = Fake::with(|w| {
        w.contents = text_contents("before");
        w.nobody_reads = true;
    });
    let outcome = fake
        .inserter()
        .insert("Olá, мир, 世界", InsertMethod::PasteKeepingText)
        .unwrap();
    assert_eq!(
        fake.steps(),
        [
            Step::Write {
                text: "Olá, мир, 世界".to_owned(),
                transient: false
            },
            Step::Paste,
        ]
    );
    assert_eq!(fake.contents(), text_contents("Olá, мир, 世界"));
    assert_eq!(outcome.method_used, InsertMethod::PasteKeepingText);
    assert!(!outcome.clipboard_restored);
    assert_eq!(outcome.diagnostic, None);
    assert!(fake.0.lock().unwrap().off_main.is_empty());
}

#[test]
fn a_failed_paste_keeping_the_text_fails_and_leaves_the_text_to_paste_by_hand() {
    let fake = Fake::with(|w| {
        w.contents = text_contents("before");
        w.fail_paste = true;
    });
    let err = fake
        .inserter()
        .insert("dictated", InsertMethod::PasteKeepingText)
        .unwrap_err();
    assert_eq!(err.kind, ErrorKind::TextInjection);
    assert_eq!(fake.contents(), text_contents("dictated"));
}

#[test]
fn new_waits_apply_from_the_next_paste_on() {
    let fake = Fake::with(|w| w.nobody_reads = true);
    let inserter = fake.inserter_with(tuning(0, 5_000, 0));
    inserter.set_tuning(tuning(0, 90, 0));
    inserter
        .insert("dictated", InsertMethod::PasteWithClipboardRestore)
        .unwrap();
    let waited = restored_after(&fake);
    assert!(waited >= Duration::from_millis(90), "{waited:?}");
    assert!(waited < Duration::from_millis(2_000), "{waited:?}");
}

/// How long after the paste keys went out the clipboard was put back.
fn restored_after(fake: &Fake) -> Duration {
    let world = fake.0.lock().unwrap();
    world.restored_at.expect("put back") - world.pasted_at.expect("keys sent")
}

#[test]
fn the_text_is_promised_and_the_keys_sent_and_the_clipboard_put_back_on_the_main_thread() {
    let fake = Fake::with(|w| w.contents = text_contents("before"));
    fake.inserter()
        .insert("dictated", InsertMethod::PasteWithClipboardRestore)
        .unwrap();
    let world = fake.0.lock().unwrap();
    assert_eq!(world.off_main, []);
    assert_eq!(world.on_main, 0, "the wait ran outside");
}

#[test]
fn the_clipboard_goes_back_once_reads_have_stopped_for_the_quiet_time() {
    let fake = Fake::with(|w| {
        w.contents = text_contents("before");
        // Each read lands well inside the quiet time of the one before, however late a busy
        // machine wakes the thread that makes it.
        w.later_reads_ms = vec![100, 200];
    });
    let outcome = fake
        .inserter_with(tuning(300, 5_000, 0))
        .insert("dictated", InsertMethod::PasteWithClipboardRestore)
        .unwrap();
    let waited = restored_after(&fake);
    assert!(
        waited >= Duration::from_millis(500),
        "waited only {waited:?}"
    );
    assert!(waited < Duration::from_millis(3_000), "waited {waited:?}");
    assert!(outcome.clipboard_restored);
    assert_eq!(outcome.diagnostic, None);
    assert_eq!(fake.contents(), text_contents("before"));
}

#[test]
fn reads_never_hold_the_clipboard_past_the_longest_wait() {
    let fake = Fake::with(|w| w.later_reads_ms = (1..40).map(|n| n * 10).collect());
    fake.inserter_with(tuning(100, 120, 0))
        .insert("dictated", InsertMethod::PasteWithClipboardRestore)
        .unwrap();
    let waited = restored_after(&fake);
    assert!(
        waited >= Duration::from_millis(120),
        "waited only {waited:?}"
    );
    assert!(waited < Duration::from_millis(300), "waited {waited:?}");
}

#[test]
fn text_nobody_reads_is_taken_back_after_the_longest_wait_and_the_outcome_says_so() {
    let fake = Fake::with(|w| {
        w.contents = text_contents("before");
        w.nobody_reads = true;
    });
    let outcome = fake
        .inserter_with(tuning(0, 150, 0))
        .insert("dictated", InsertMethod::PasteWithClipboardRestore)
        .unwrap();
    let waited = restored_after(&fake);
    assert!(
        waited >= Duration::from_millis(150),
        "waited only {waited:?}"
    );
    assert_eq!(fake.contents(), text_contents("before"));
    assert!(outcome.clipboard_restored);
    assert_eq!(outcome.paste_read, Some(PasteRead::Unread));
    assert_eq!(outcome.diagnostic, None);
}

#[test]
fn after_keys_that_failed_to_go_out_the_clipboard_goes_back_after_the_short_wait() {
    let fake = Fake::with(|w| {
        w.contents = text_contents("before");
        w.fail_paste = true;
    });
    let err = fake
        .inserter_with(tuning(0, 5_000, 80))
        .insert("dictated", InsertMethod::PasteWithClipboardRestore)
        .unwrap_err();
    assert_eq!(err.kind, ErrorKind::TextInjection);
    let waited = restored_after(&fake);
    assert!(
        waited >= Duration::from_millis(80),
        "waited only {waited:?}"
    );
    assert!(waited < Duration::from_millis(2_000), "waited {waited:?}");
    assert_eq!(fake.contents(), text_contents("before"));
}

#[test]
fn a_clipboard_written_by_something_else_during_the_wait_is_left_alone_at_once() {
    let fake = Fake::with(|w| {
        w.contents = text_contents("before");
        w.nobody_reads = true;
        w.replaced_after_ms = Some(30);
    });
    let started = std::time::Instant::now();
    let outcome = fake
        .inserter_with(tuning(0, 5_000, 0))
        .insert("dictated", InsertMethod::PasteWithClipboardRestore)
        .unwrap();
    assert!(started.elapsed() < Duration::from_millis(2_000));
    assert_eq!(fake.contents(), text_contents("copied by the user"));
    assert!(
        !fake
            .steps()
            .iter()
            .any(|step| matches!(step, Step::Restore(_)))
    );
    assert!(!outcome.clipboard_restored);
    assert!(outcome.diagnostic.unwrap().contains("left as it is"));
}

#[test]
fn a_new_owner_the_change_count_misses_still_leaves_the_clipboard_alone() {
    let fake = Fake::with(|w| {
        w.contents = text_contents("before");
        w.later_reads_ms = vec![20];
        w.owner_changed_after_ms = Some(40);
    });
    let outcome = fake
        .inserter_with(tuning(200, 5_000, 0))
        .insert("dictated", InsertMethod::PasteWithClipboardRestore)
        .unwrap();
    assert!(!outcome.clipboard_restored);
    assert!(
        !fake
            .steps()
            .iter()
            .any(|step| matches!(step, Step::Restore(_)))
    );
}
