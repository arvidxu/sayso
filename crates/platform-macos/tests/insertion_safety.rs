mod insertion_support;

use std::sync::Arc;
use std::time::{Duration, Instant};

use dictation_api::TextInjector;
use dictation_types::{ErrorKind, InsertMethod, PasteRead, PermissionKind};
use insertion_support::*;

#[test]
fn without_permission_to_send_keys_pasting_and_typing_fail_before_touching_anything() {
    for method in [
        InsertMethod::PasteWithClipboardRestore,
        InsertMethod::PasteKeepingText,
        InsertMethod::SimulatedTyping,
    ] {
        let fake = Fake::with(|w| {
            w.contents = text_contents("before");
            w.keys_dropped = true;
        });
        let err = fake.inserter().insert("dictated", method).unwrap_err();
        assert_eq!(
            err.kind,
            ErrorKind::PermissionDenied(Some(PermissionKind::Accessibility)),
            "{method}"
        );
        assert_eq!(fake.steps(), [], "{method}");
        assert_eq!(fake.contents(), text_contents("before"));
    }
}

#[test]
fn a_failed_write_puts_the_saved_clipboard_back_and_fails() {
    let fake = Fake::with(|w| {
        w.contents = image_and_text();
        w.fail_write = true;
    });
    let err = fake
        .inserter()
        .insert("dictated", InsertMethod::PasteWithClipboardRestore)
        .unwrap_err();
    assert_eq!(err.kind, ErrorKind::TextInjection);
    assert_eq!(fake.contents(), image_and_text());
    assert_eq!(
        fake.steps(),
        [Step::Read, Step::Restore(image_and_text())],
        "nothing was pasted"
    );
}

#[test]
fn an_unreadable_clipboard_is_pasted_over_but_never_put_back_as_empty() {
    let fake = Fake::with(|w| {
        w.contents = text_contents("kept from us");
        w.unreadable = true;
    });
    let outcome = fake
        .inserter()
        .insert("dictated", InsertMethod::PasteWithClipboardRestore)
        .unwrap();
    assert_eq!(
        fake.steps(),
        [Step::Read, Step::Offer("dictated".to_owned()), Step::Paste,]
    );
    assert!(!outcome.clipboard_restored);
    assert!(outcome.diagnostic.unwrap().contains("could not be read"));
}

#[test]
fn a_failed_paste_over_an_unreadable_clipboard_does_not_empty_it() {
    let fake = Fake::with(|w| {
        w.contents = text_contents("kept from us");
        w.unreadable = true;
        w.fail_paste = true;
    });
    let err = fake
        .inserter()
        .insert("dictated", InsertMethod::PasteWithClipboardRestore)
        .unwrap_err();
    assert!(err.message.contains("not put back"), "{}", err.message);
    assert!(
        !fake
            .steps()
            .iter()
            .any(|step| matches!(step, Step::Restore(_)))
    );
    assert_eq!(
        fake.contents(),
        text_contents("dictated"),
        "the dictated text stays on it: what was there could not be read, so it is lost"
    );
}

#[test]
fn a_partly_unreadable_clipboard_gets_back_what_was_read_and_says_the_rest_was_not() {
    let fake = Fake::with(|w| {
        w.contents = image_and_text();
        w.partly_unreadable = true;
    });
    let outcome = fake
        .inserter()
        .insert("dictated", InsertMethod::PasteWithClipboardRestore)
        .unwrap();
    assert_eq!(fake.contents(), image_and_text());
    assert_eq!(fake.steps().last(), Some(&Step::Restore(image_and_text())));
    assert!(outcome.clipboard_restored);
    assert!(outcome.diagnostic.unwrap().contains("could not be read"));
}

#[test]
fn two_pastes_at_once_each_put_back_the_clipboard_from_before_both() {
    let fake = Fake::with(|w| {
        w.contents = text_contents("before");
        w.paste_takes = Duration::from_millis(50);
    });
    let inserter = Arc::new(fake.inserter());
    let pastes: Vec<_> = ["first", "second"]
        .into_iter()
        .map(|text| {
            let inserter = Arc::clone(&inserter);
            std::thread::spawn(move || {
                inserter
                    .insert(text, InsertMethod::PasteWithClipboardRestore)
                    .unwrap()
            })
        })
        .collect();
    for paste in pastes {
        assert!(paste.join().unwrap().clipboard_restored);
    }
    assert_eq!(fake.contents(), text_contents("before"));
    let restored: Vec<_> = fake
        .steps()
        .into_iter()
        .filter_map(|step| match step {
            Step::Restore(contents) => Some(contents),
            _ => None,
        })
        .collect();
    assert_eq!(restored, [text_contents("before"), text_contents("before")]);
}

#[test]
fn a_read_paste_stops_waiting_for_quiet_as_soon_as_the_next_one_is_asked_for() {
    let fake = Fake::with(|w| w.contents = text_contents("before"));
    let quiet_ms = 3_000;
    let inserter = Arc::new(fake.inserter_with(tuning(quiet_ms, 5_000, 0)));
    let first = {
        let inserter = Arc::clone(&inserter);
        std::thread::spawn(move || {
            let outcome = inserter
                .insert("first", InsertMethod::PasteWithClipboardRestore)
                .unwrap();
            (outcome, Instant::now())
        })
    };
    while !fake.steps().contains(&Step::Paste) {
        std::thread::sleep(Duration::from_millis(5));
    }
    let second_started = Instant::now();
    let second = inserter
        .insert("second", InsertMethod::PasteKeepingText)
        .unwrap();
    let (first, first_done) = first.join().unwrap();

    let held_back = first_done.saturating_duration_since(second_started);
    assert!(
        held_back < Duration::from_millis(quiet_ms / 2),
        "{held_back:?}"
    );
    assert!(first.clipboard_restored);
    assert_eq!(first.paste_read, Some(PasteRead::Read));
    assert_eq!(first.diagnostic, None);
    assert_eq!(second.paste_read, None);
    assert_eq!(second.diagnostic, None);
    assert_eq!(fake.contents(), text_contents("second"));
    let steps = fake.steps();
    let restored = steps
        .iter()
        .position(|step| *step == Step::Restore(text_contents("before")))
        .expect("the first paste put the clipboard back");
    let written = steps
        .iter()
        .position(|step| matches!(step, Step::Write { text, .. } if text == "second"))
        .expect("the second paste wrote its text");
    assert!(restored < written);
}

#[test]
fn an_unread_paste_keeps_the_next_one_waiting_until_its_text_is_read() {
    let read_after_ms = 300;
    let fake = Fake::with(|w| {
        w.contents = text_contents("before");
        w.nobody_reads = true;
        w.later_reads_ms = vec![read_after_ms];
    });
    let inserter = Arc::new(fake.inserter_with(tuning(2_000, 5_000, 0)));
    let first = {
        let inserter = Arc::clone(&inserter);
        std::thread::spawn(move || {
            inserter
                .insert("first", InsertMethod::PasteWithClipboardRestore)
                .unwrap()
        })
    };
    while !fake.steps().contains(&Step::Paste) {
        std::thread::sleep(Duration::from_millis(5));
    }
    let first_pasted = fake.0.lock().unwrap().pasted_at.expect("keys sent");
    inserter
        .insert("second", InsertMethod::PasteKeepingText)
        .unwrap();
    let first = first.join().unwrap();

    let waited = fake.0.lock().unwrap().restored_at.expect("put back") - first_pasted;
    // Held until the read, then let go without the quiet wait.
    assert!(waited >= Duration::from_millis(read_after_ms), "{waited:?}");
    assert!(waited < Duration::from_millis(1_500), "{waited:?}");
    assert_eq!(first.paste_read, Some(PasteRead::Read));
    assert_eq!(fake.contents(), text_contents("second"));
}

#[test]
fn pastes_go_in_the_order_they_were_asked_for() {
    // Each paste takes long enough that every later one is asked for while it runs, and the
    // requests are far enough apart that a busy machine starting a thread late keeps their order.
    let fake = Fake::with(|w| w.paste_takes = Duration::from_millis(700));
    let inserter = Arc::new(fake.inserter());
    let texts: Vec<String> = (0..5).map(|n| format!("part {n}")).collect();
    let pastes: Vec<_> = texts
        .iter()
        .map(|text| {
            let inserter = Arc::clone(&inserter);
            let text = text.clone();
            let paste = std::thread::spawn(move || {
                inserter
                    .insert(&text, InsertMethod::PasteKeepingText)
                    .unwrap();
            });
            std::thread::sleep(Duration::from_millis(120));
            paste
        })
        .collect();
    for paste in pastes {
        paste.join().unwrap();
    }
    let written: Vec<String> = fake
        .steps()
        .into_iter()
        .filter_map(|step| match step {
            Step::Write { text, .. } => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(written, texts);
}

#[test]
fn pasting_is_refused_on_the_main_thread_before_touching_anything() {
    for method in [
        InsertMethod::PasteWithClipboardRestore,
        InsertMethod::PasteKeepingText,
    ] {
        let fake = Fake::with(|w| {
            w.contents = text_contents("before");
            w.caller_is_main = true;
        });
        let err = fake.inserter().insert("dictated", method).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Internal, "{method}");
        assert_eq!(fake.steps(), [], "{method}");
        assert_eq!(fake.contents(), text_contents("before"));
    }
}
