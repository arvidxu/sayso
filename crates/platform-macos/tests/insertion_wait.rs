use std::time::{Duration, Instant};

use platform_macos::insertion::{WaitStep, wait_step};
use platform_macos::{InsertionTuning, OfferNews};

const TUNING: InsertionTuning = InsertionTuning {
    restore_quiet_after_read_ms: 200,
    restore_unread_timeout_ms: 8_000,
    restore_after_failed_paste_ms: 500,
};

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

fn read_at(at: Instant) -> OfferNews {
    OfferNews {
        last_read: Some(at),
        ..OfferNews::default()
    }
}

#[test]
fn unread_text_waits_for_the_longest_wait_or_the_short_one_after_failed_keys() {
    let start = Instant::now();
    let none = OfferNews::default();
    assert_eq!(
        wait_step(&TUNING, &none, start, start + ms(1_000), true),
        WaitStep::Wait(ms(7_000))
    );
    assert_eq!(
        wait_step(&TUNING, &none, start, start + ms(8_000), true),
        WaitStep::Done
    );
    assert_eq!(
        wait_step(&TUNING, &none, start, start + ms(100), false),
        WaitStep::Wait(ms(400))
    );
    assert_eq!(
        wait_step(&TUNING, &none, start, start + ms(500), false),
        WaitStep::Done
    );
}

#[test]
fn read_text_waits_for_quiet_after_the_last_read_within_the_longest_wait() {
    let start = Instant::now();
    let news = read_at(start + ms(1_000));
    assert_eq!(
        wait_step(&TUNING, &news, start, start + ms(1_050), true),
        WaitStep::Wait(ms(150))
    );
    assert_eq!(
        wait_step(&TUNING, &news, start, start + ms(1_200), true),
        WaitStep::Done
    );
    let late = read_at(start + ms(7_900));
    assert_eq!(
        wait_step(&TUNING, &late, start, start + ms(7_950), true),
        WaitStep::Wait(ms(50))
    );
    assert_eq!(
        wait_step(&TUNING, &late, start, start + ms(8_000), true),
        WaitStep::Done
    );
}

#[test]
fn a_clipboard_taken_over_ends_the_wait_at_once() {
    let start = Instant::now();
    let news = OfferNews {
        replaced: true,
        ..read_at(start)
    };
    assert_eq!(
        wait_step(&TUNING, &news, start, start, true),
        WaitStep::Done
    );
}

#[test]
fn reads_count_only_once_the_keys_are_about_to_go_out() {
    let offer = platform_macos::TextOffer::new();
    offer.note_read();
    assert_eq!(
        offer.news(),
        OfferNews {
            taken_before_keys: true,
            ..OfferNews::default()
        }
    );
    offer.start_counting();
    offer.note_read();
    let news = offer.news();
    assert!(news.last_read.is_some());
    assert!(!news.replaced);
    let started = Instant::now();
    offer.wait_for_news(&OfferNews::default(), ms(5_000));
    assert!(
        started.elapsed() < ms(1_000),
        "news since then returns at once"
    );
}

#[test]
fn another_paste_ends_the_wait_at_once_only_after_the_text_was_read() {
    let start = Instant::now();
    let unread = OfferNews {
        superseded: true,
        ..OfferNews::default()
    };
    assert_eq!(
        wait_step(&TUNING, &unread, start, start + ms(10), true),
        WaitStep::Wait(ms(7_990))
    );
    let read = OfferNews {
        superseded: true,
        ..read_at(start + ms(5))
    };
    assert_eq!(
        wait_step(&TUNING, &read, start, start + ms(10), true),
        WaitStep::Done
    );
}
