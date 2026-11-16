//! Most checks use an unwatched source fed through `handle_input`, so they never depend on
//! what the user types meanwhile.

mod support;

use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Barrier, Mutex, Weak};
use std::thread::{self, ThreadId};
use std::time::{Duration, Instant};

use dictation_api::{HotkeySource, ShortcutListener};
use dictation_types::{HotkeyNotice, ModifierKey, ShortcutState};
use platform_macos::keys::vk;
use platform_macos::{InputEvent, InputKind, MacosHotkeySource};
use support::*;

/// How long to wait for a notice that should come.
const ARRIVES: Duration = Duration::from_secs(5);
/// How long to wait to be fairly sure a notice that should not come does not.
const NOTHING: Duration = Duration::from_millis(100);

fn right_option_down() -> InputEvent {
    InputEvent::new(
        InputKind::FlagsChanged {
            key: vk::RIGHT_OPTION,
        },
        RIGHT_OPTION,
        10,
    )
}

fn right_option_up() -> InputEvent {
    InputEvent::new(
        InputKind::FlagsChanged {
            key: vk::RIGHT_OPTION,
        },
        0,
        20,
    )
}

/// A listener that sends each notice, with the thread it arrived on, to the returned receiver.
fn channel_listener() -> (Receiver<(HotkeyNotice, ThreadId)>, ShortcutListener) {
    let (sender, receiver) = channel();
    (
        receiver,
        Box::new(move |notice| {
            let _ = sender.send((notice, thread::current().id()));
        }),
    )
}

/// A listener that records into a list, for checking what has arrived at one moment.
fn recording() -> (Arc<Mutex<Vec<HotkeyNotice>>>, ShortcutListener) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    (
        seen,
        Box::new(move |notice| sink.lock().unwrap().push(notice)),
    )
}

fn state_of(notice: &HotkeyNotice) -> Option<ShortcutState> {
    match notice {
        HotkeyNotice::Shortcut(event) => Some(event.state),
        HotkeyNotice::WatchingStopped => None,
    }
}

fn talk_source() -> MacosHotkeySource {
    let source = MacosHotkeySource::unwatched();
    source
        .register(lone("talk", ModifierKey::RightAlt))
        .unwrap();
    source
}

#[test]
fn bindings_are_kept_listed_and_removed() {
    let source = MacosHotkeySource::new();
    source
        .register(lone("talk", ModifierKey::RightAlt))
        .unwrap();
    source
        .register(lone_toggle("other", ModifierKey::Fn))
        .unwrap();
    assert_eq!(source.list_bindings().unwrap().len(), 2);
    assert!(source.unregister(&id("talk")).unwrap());
    assert!(!source.unregister(&id("talk")).unwrap());
    source.clear().unwrap();
    assert!(source.list_bindings().unwrap().is_empty());
}

#[test]
fn the_listener_hears_presses_in_order_on_a_thread_of_its_own() {
    let source = talk_source();
    let (heard, listener) = channel_listener();
    source.listen(listener).unwrap();
    assert!(source.is_listening());
    source.handle_input(right_option_down());
    source.handle_input(right_option_up());
    let (first, first_thread) = heard.recv_timeout(ARRIVES).unwrap();
    let (second, second_thread) = heard.recv_timeout(ARRIVES).unwrap();
    assert_eq!(state_of(&first), Some(ShortcutState::Pressed));
    assert_eq!(state_of(&second), Some(ShortcutState::Released));
    assert_ne!(first_thread, thread::current().id());
    assert_eq!(first_thread, second_thread, "one delivery thread");
}

#[test]
fn a_binding_change_reaches_the_listener_on_its_thread_not_the_callers() {
    let source = talk_source();
    let (heard, listener) = channel_listener();
    source.listen(listener).unwrap();
    source.handle_input(right_option_down());
    source.unregister(&id("talk")).unwrap();
    let (pressed, delivery) = heard.recv_timeout(ARRIVES).unwrap();
    let (interrupted, thread) = heard.recv_timeout(ARRIVES).unwrap();
    assert_eq!(state_of(&pressed), Some(ShortcutState::Pressed));
    assert_eq!(state_of(&interrupted), Some(ShortcutState::Interrupted));
    assert_eq!(thread, delivery);
    assert_ne!(thread, thread::current().id());
}

#[test]
fn stopping_delivers_the_interruption_before_returning_and_nothing_after() {
    let source = talk_source();
    let (seen, listener) = recording();
    source.listen(listener).unwrap();
    source.handle_input(right_option_down());
    source.stop_listening().unwrap();
    let states: Vec<_> = seen.lock().unwrap().iter().map(state_of).collect();
    assert_eq!(
        states,
        [
            Some(ShortcutState::Pressed),
            Some(ShortcutState::Interrupted)
        ],
        "everything queued is delivered by the time stopping returns"
    );
    assert!(!source.is_listening());
    source.handle_input(right_option_up());
    source.handle_input(right_option_down());
    source.unregister(&id("talk")).unwrap();
    thread::sleep(NOTHING);
    assert_eq!(seen.lock().unwrap().len(), 2, "nothing after stopping");
}

#[test]
fn a_slow_listener_does_not_hold_up_the_input() {
    let source = talk_source();
    let (sender, heard) = channel();
    source
        .listen(Box::new(move |notice| {
            thread::sleep(Duration::from_millis(300));
            let _ = sender.send(notice);
        }))
        .unwrap();
    let started = Instant::now();
    source.handle_input(right_option_down());
    source.handle_input(right_option_up());
    assert!(started.elapsed() < Duration::from_millis(250));
    heard.recv_timeout(ARRIVES).unwrap();
    heard.recv_timeout(ARRIVES).unwrap();
}

#[test]
fn the_listener_can_change_bindings_and_stop_listening_from_inside_its_call() {
    let source = Arc::new(talk_source());
    let weak: Weak<MacosHotkeySource> = Arc::downgrade(&source);
    let (sender, heard) = channel();
    source
        .listen(Box::new(move |notice| {
            if let Some(source) = weak.upgrade() {
                source
                    .register(lone_toggle("other", ModifierKey::Fn))
                    .unwrap();
                source.stop_listening().unwrap();
            }
            let _ = sender.send(notice);
        }))
        .unwrap();
    source.handle_input(right_option_down());
    let notice = heard.recv_timeout(ARRIVES).unwrap();
    assert_eq!(state_of(&notice), Some(ShortcutState::Pressed));
    // Stopping from inside interrupted the press; that arrives after the call returned.
    let notice = heard.recv_timeout(ARRIVES).unwrap();
    assert_eq!(state_of(&notice), Some(ShortcutState::Interrupted));
    assert!(heard.recv_timeout(NOTHING).is_err(), "nothing after that");
    assert_eq!(source.list_bindings().unwrap().len(), 2);
    assert!(!source.is_listening());
}

#[test]
fn listening_again_hands_later_notices_to_the_new_listener() {
    let source = talk_source();
    let (first, listener) = channel_listener();
    source.listen(listener).unwrap();
    source.handle_input(right_option_down());
    let (second, listener) = channel_listener();
    source.listen(listener).unwrap();
    source.handle_input(right_option_up());
    let (pressed, _) = first.recv_timeout(ARRIVES).unwrap();
    assert_eq!(state_of(&pressed), Some(ShortcutState::Pressed));
    let (released, _) = second.recv_timeout(ARRIVES).unwrap();
    assert_eq!(state_of(&released), Some(ShortcutState::Released));
    assert!(first.recv_timeout(NOTHING).is_err());
}

#[test]
fn the_watching_stopped_hook_interrupts_a_press_and_tells_the_listener() {
    let source = talk_source();
    let (heard, listener) = channel_listener();
    source.listen(listener).unwrap();
    source.handle_input(right_option_down());
    source.watching_stopped();
    let notices: Vec<_> = (0..3)
        .map(|_| heard.recv_timeout(ARRIVES).unwrap().0)
        .collect();
    assert_eq!(state_of(&notices[0]), Some(ShortcutState::Pressed));
    assert_eq!(state_of(&notices[1]), Some(ShortcutState::Interrupted));
    assert_eq!(notices[2], HotkeyNotice::WatchingStopped);
    // The press was interrupted, so its release says nothing.
    source.handle_input(right_option_up());
    assert!(heard.recv_timeout(NOTHING).is_err());
}

#[test]
fn a_listener_told_watching_stopped_is_no_longer_listening_and_can_listen_again() {
    let source = Arc::new(talk_source());
    let weak: Weak<MacosHotkeySource> = Arc::downgrade(&source);
    let (again, again_listener) = channel_listener();
    let mut again_listener = Some(again_listener);
    let (sender, checks) = channel();
    source
        .listen(Box::new(move |notice| {
            let HotkeyNotice::WatchingStopped = notice else {
                return;
            };
            let Some(source) = weak.upgrade() else {
                return;
            };
            let before = source.is_listening();
            let listened = again_listener
                .take()
                .map(|listener| source.listen(listener).is_ok());
            let after = source.is_listening();
            let _ = sender.send((before, listened, after));
        }))
        .unwrap();
    assert!(source.is_listening());
    source.watching_stopped();
    let (before, listened, after) = checks.recv_timeout(ARRIVES).unwrap();
    assert!(
        !before,
        "watching has ended by the time the listener hears it"
    );
    assert_eq!(listened, Some(true));
    assert!(after, "listening again watches anew");
    source.handle_input(right_option_down());
    let (pressed, _) = again.recv_timeout(ARRIVES).unwrap();
    assert_eq!(state_of(&pressed), Some(ShortcutState::Pressed));
}

#[test]
fn listening_and_stopping_at_once_end_either_listening_with_a_listener_or_not_at_all() {
    for _ in 0..20 {
        let source = Arc::new(talk_source());
        let (seen, listener) = recording();
        let start = Arc::new(Barrier::new(2));
        let listening = {
            let (source, start) = (Arc::clone(&source), Arc::clone(&start));
            thread::spawn(move || {
                start.wait();
                source.listen(listener).unwrap();
            })
        };
        let stopping = {
            let (source, start) = (Arc::clone(&source), Arc::clone(&start));
            thread::spawn(move || {
                start.wait();
                source.stop_listening().unwrap();
            })
        };
        listening.join().unwrap();
        stopping.join().unwrap();
        let listening = source.is_listening();
        source.handle_input(right_option_down());
        // Stopping now waits for everything queued to be delivered.
        source.stop_listening().unwrap();
        let heard_press = seen
            .lock()
            .unwrap()
            .iter()
            .any(|notice| state_of(notice) == Some(ShortcutState::Pressed));
        assert_eq!(heard_press, listening);
    }
}

/// Installs a real keyboard tap when the test run has the Accessibility permission, and
/// otherwise checks the refusal. Whatever the user types meanwhile may reach the listener, so
/// only what holds either way is checked: the outcome, and that nothing arrives once stopped.
#[cfg(target_os = "macos")]
#[test]
fn listening_for_real_starts_or_is_refused_for_accessibility_and_stopping_ends_it() {
    let source = MacosHotkeySource::new();
    source
        .register(lone("talk", ModifierKey::RightAlt))
        .unwrap();
    let (heard, listener) = channel_listener();
    match source.listen(listener) {
        Ok(()) => {
            assert!(source.is_listening());
            source.stop_listening().unwrap();
            assert!(!source.is_listening());
            while heard.try_recv().is_ok() {}
        }
        Err(err) => {
            assert_eq!(
                err.kind,
                dictation_types::ErrorKind::PermissionDenied(Some(
                    dictation_types::PermissionKind::Accessibility
                ))
            );
            assert!(!source.is_listening());
        }
    }
    source.handle_input(right_option_down());
    source.handle_input(right_option_up());
    assert!(
        heard.recv_timeout(NOTHING).is_err(),
        "a stopped or refused listener hears nothing"
    );
}

#[test]
fn stopping_when_not_listening_is_not_an_error() {
    let source = MacosHotkeySource::new();
    source.stop_listening().unwrap();
    source.stop_listening().unwrap();
}
