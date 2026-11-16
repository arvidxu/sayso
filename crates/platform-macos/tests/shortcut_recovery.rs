mod support;

use std::sync::{Arc, Mutex};

use dictation_types::{KeyCode, ModifierKey, ShortcutState};
use platform_macos::keys::{VirtualKey, flags, vk};
use platform_macos::{Disposition, InputKind, KeyState, ShortcutMatcher};
use support::*;

/// Key state the test sets: which keys and buttons the "hardware" says are down.
#[derive(Clone, Default)]
struct Hardware(Arc<Mutex<(Vec<VirtualKey>, Vec<i64>)>>);

impl KeyState for Hardware {
    fn key_down(&self, key: VirtualKey) -> bool {
        self.0.lock().unwrap().0.contains(&key)
    }

    fn button_down(&self, button: i64) -> bool {
        self.0.lock().unwrap().1.contains(&button)
    }
}

fn with_hardware(bindings: Vec<dictation_types::HotkeyBinding>) -> (Keys, Hardware) {
    let hardware = Hardware::default();
    let mut matcher = ShortcutMatcher::with_key_state(Box::new(hardware.clone()));
    for binding in bindings {
        matcher.register(binding).unwrap();
    }
    (
        Keys {
            matcher,
            now: 1_000,
        },
        hardware,
    )
}

#[test]
fn a_key_release_never_seen_does_not_block_a_lone_modifier_for_good() {
    let (mut keys, hardware) = with_hardware(vec![lone("talk", ModifierKey::RightAlt)]);
    hardware.0.lock().unwrap().0.push(KEY_A);
    keys.down(KEY_A, 0);
    let (events, _) = keys.modifier(vk::RIGHT_OPTION, RIGHT_OPTION);
    assert!(events.is_empty(), "the key really is down");
    keys.modifier(vk::RIGHT_OPTION, 0);

    // The release of the key is lost; the hardware says it is up.
    hardware.0.lock().unwrap().0.clear();
    let (events, _) = keys.modifier(vk::RIGHT_OPTION, RIGHT_OPTION);
    assert_eq!(states(&events), one("talk", ShortcutState::Pressed));
}

#[test]
fn a_mouse_release_never_seen_does_not_block_a_lone_modifier_for_good() {
    let (mut keys, _hardware) = with_hardware(vec![
        lone("talk", ModifierKey::RightAlt),
        lone("mouse", ModifierKey::MouseSideBack),
    ]);
    keys.mouse_down(3);
    keys.matcher.interrupt(keys.now);
    // The mouse release is lost; the hardware says no button is down.
    let (events, _) = keys.modifier(vk::RIGHT_OPTION, RIGHT_OPTION);
    assert_eq!(states(&events), one("talk", ShortcutState::Pressed));
}

#[test]
fn a_fresh_press_of_a_button_whose_release_was_missed_presses_again() {
    let mut keys = Keys::with(vec![lone("mouse", ModifierKey::MouseSideBack)]);
    keys.mouse_down(3);
    keys.matcher.interrupt(keys.now);
    let (events, disposition) = keys.mouse_down(3);
    assert_eq!(states(&events), one("mouse", ShortcutState::Pressed));
    assert_eq!(disposition, Disposition::Swallow);
}

#[test]
fn after_missed_events_a_held_shortcut_key_is_still_kept_from_the_application() {
    let mut keys = Keys::with(vec![combo(
        "talk",
        &[ModifierKey::LeftControl],
        KeyCode::Space,
    )]);
    keys.modifier(vk::CONTROL, LEFT_CONTROL);
    keys.down(SPACE, LEFT_CONTROL);
    let interrupted = keys.matcher.interrupt(keys.now).unwrap();
    assert_eq!(interrupted.state, ShortcutState::Interrupted);
    assert_eq!(
        keys.repeat(SPACE, LEFT_CONTROL),
        (vec![], Disposition::Swallow)
    );
    assert_eq!(keys.up(SPACE, LEFT_CONTROL), (vec![], Disposition::Swallow));
    assert_eq!(keys.down(SPACE, 0), (vec![], Disposition::PassThrough));
}

#[test]
fn after_missed_events_a_held_mouse_button_is_still_kept_including_drags() {
    let mut keys = Keys::with(vec![lone("mouse", ModifierKey::MouseSideForward)]);
    keys.mouse_down(4);
    keys.matcher.interrupt(keys.now);
    let drag = keys.send(InputKind::MouseDragged { button: 4 }, 0);
    assert_eq!(drag, (vec![], Disposition::Swallow));
    assert_eq!(keys.mouse_up(4), (vec![], Disposition::Swallow));
}

#[test]
fn drags_of_a_shortcut_button_are_kept_and_other_drags_go_through() {
    let mut keys = Keys::with(vec![lone("mouse", ModifierKey::MouseSideForward)]);
    keys.mouse_down(4);
    assert_eq!(
        keys.send(InputKind::MouseDragged { button: 4 }, 0),
        (vec![], Disposition::Swallow)
    );
    assert_eq!(
        keys.send(InputKind::MouseDragged { button: 2 }, 0),
        (vec![], Disposition::PassThrough)
    );
    keys.mouse_up(4);
    assert_eq!(
        keys.send(InputKind::MouseDragged { button: 4 }, 0),
        (vec![], Disposition::PassThrough)
    );
}

#[test]
fn registering_the_same_binding_again_while_down_keeps_the_press() {
    let mut keys = Keys::with(vec![lone("talk", ModifierKey::RightAlt)]);
    keys.modifier(vk::RIGHT_OPTION, RIGHT_OPTION);
    keys.matcher
        .register(lone_toggle("talk", ModifierKey::RightAlt))
        .unwrap();
    assert!(keys.matcher.take_pending().is_empty());
    let (events, _) = keys.modifier(vk::RIGHT_OPTION, 0);
    assert_eq!(states(&events), one("talk", ShortcutState::Released));
    assert_eq!(
        keys.matcher.bindings()[0].trigger_mode,
        dictation_types::HotkeyTriggerMode::ToggleOnPress
    );
}

#[test]
fn moving_a_shortcut_to_other_keys_while_down_interrupts_it() {
    let mut keys = Keys::with(vec![lone("talk", ModifierKey::RightAlt)]);
    keys.modifier(vk::RIGHT_OPTION, RIGHT_OPTION);
    keys.matcher
        .register(lone("talk", ModifierKey::Fn))
        .unwrap();
    assert_eq!(
        states(&keys.matcher.take_pending()),
        one("talk", ShortcutState::Interrupted)
    );
}

#[test]
fn a_function_key_reached_through_fn_matches_a_shortcut_without_fn() {
    let f5 = 0x60;
    let mut keys = Keys::with(vec![combo("talk", &[], KeyCode::Function(5))]);
    keys.modifier(vk::FUNCTION, flags::FN);
    let (events, disposition) = keys.down(f5, flags::FN);
    assert_eq!(states(&events), one("talk", ShortcutState::Pressed));
    assert_eq!(disposition, Disposition::Swallow);

    // The same for a navigation key: Fn + Left is Home.
    let mut keys = Keys::with(vec![combo(
        "talk",
        &[ModifierKey::LeftControl],
        KeyCode::Named("Home".to_owned()),
    )]);
    keys.modifier(vk::CONTROL, LEFT_CONTROL);
    keys.modifier(vk::FUNCTION, LEFT_CONTROL | flags::FN);
    let (events, _) = keys.down(vk::HOME, LEFT_CONTROL | flags::FN);
    assert_eq!(states(&events), one("talk", ShortcutState::Pressed));
}

#[test]
fn fn_held_still_counts_for_an_ordinary_key() {
    let mut keys = Keys::with(vec![combo(
        "talk",
        &[ModifierKey::LeftControl],
        KeyCode::Space,
    )]);
    keys.modifier(vk::CONTROL, LEFT_CONTROL);
    keys.modifier(vk::FUNCTION, LEFT_CONTROL | flags::FN);
    let (events, disposition) = keys.down(SPACE, LEFT_CONTROL | flags::FN);
    assert!(events.is_empty());
    assert_eq!(disposition, Disposition::PassThrough);
}

#[test]
fn after_missed_events_the_next_press_of_a_lone_modifier_is_not_lost() {
    let mut keys = Keys::with(vec![lone("talk", ModifierKey::RightAlt)]);
    keys.modifier(vk::RIGHT_OPTION, RIGHT_OPTION);
    keys.matcher.interrupt(keys.now);
    // The release of Right Option is missed; the next event is the next press.
    let (events, _) = keys.modifier(vk::RIGHT_OPTION, RIGHT_OPTION);
    assert_eq!(states(&events), one("talk", ShortcutState::Pressed));
    let (events, _) = keys.modifier(vk::RIGHT_OPTION, 0);
    assert_eq!(states(&events), one("talk", ShortcutState::Released));
}

#[test]
fn after_missed_events_the_next_caps_lock_press_is_not_lost() {
    let mut keys = Keys::with(vec![lone_toggle("toggle", ModifierKey::CapsLock)]);
    keys.modifier(vk::CAPS_LOCK, flags::CAPS_LOCK);
    keys.matcher.interrupt(keys.now);
    // Two presses are missed, so the lock is back where it was when the next one comes.
    let (events, _) = keys.modifier(vk::CAPS_LOCK, flags::CAPS_LOCK);
    assert_eq!(
        states(&events),
        vec![
            ("toggle".to_owned(), ShortcutState::Pressed),
            ("toggle".to_owned(), ShortcutState::Released)
        ]
    );
}

#[test]
fn a_main_button_the_system_says_is_down_keeps_a_lone_modifier_up() {
    let (mut keys, hardware) = with_hardware(vec![lone("talk", ModifierKey::RightAlt)]);
    // A drag that began before watching did, so neither its press nor a drag was seen.
    hardware.0.lock().unwrap().1.push(0);
    let (events, _) = keys.modifier(vk::RIGHT_OPTION, RIGHT_OPTION);
    assert!(events.is_empty(), "the main button is held");
    keys.modifier(vk::RIGHT_OPTION, 0);
    hardware.0.lock().unwrap().1.clear();
    let (events, _) = keys.modifier(vk::RIGHT_OPTION, RIGHT_OPTION);
    assert_eq!(states(&events), one("talk", ShortcutState::Pressed));
    keys.modifier(vk::RIGHT_OPTION, 0);

    // A main button whose release is lost: the system says it is up.
    keys.mouse_down(0);
    let (events, _) = keys.modifier(vk::RIGHT_OPTION, RIGHT_OPTION);
    assert_eq!(states(&events), one("talk", ShortcutState::Pressed));
}

#[test]
fn a_button_unbound_after_its_release_was_missed_goes_through_on_its_next_press() {
    let mut keys = Keys::with(vec![lone("mouse", ModifierKey::MouseSideBack)]);
    keys.mouse_down(3);
    keys.matcher.interrupt(keys.now);
    keys.matcher.unregister(&id("mouse"));
    assert_eq!(keys.mouse_down(3), (vec![], Disposition::PassThrough));
    assert_eq!(
        keys.send(InputKind::MouseDragged { button: 3 }, 0),
        (vec![], Disposition::PassThrough),
        "its drags reach the application like its press"
    );
    assert_eq!(
        keys.mouse_up(3),
        (vec![], Disposition::PassThrough),
        "and so does its release"
    );
}
