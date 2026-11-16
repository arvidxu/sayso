mod support;

use dictation_types::{KeyCode, ModifierKey, ShortcutState};
use platform_macos::keys::{flags, vk};
use platform_macos::{Disposition, InputKind};
use support::*;

fn right_option() -> Keys {
    Keys::with(vec![lone("talk", ModifierKey::RightAlt)])
}

#[test]
fn pressing_and_releasing_the_modifier_alone_presses_and_releases() {
    let mut keys = right_option();
    let (events, disposition) = keys.modifier(vk::RIGHT_OPTION, RIGHT_OPTION);
    assert_eq!(states(&events), one("talk", ShortcutState::Pressed));
    assert_eq!(events[0].timestamp_ms, keys.now, "the time of the input");
    assert_eq!(disposition, Disposition::PassThrough);
    let (events, disposition) = keys.modifier(vk::RIGHT_OPTION, 0);
    assert_eq!(states(&events), one("talk", ShortcutState::Released));
    assert_eq!(disposition, Disposition::PassThrough);
    let (events, _) = keys.modifier(vk::OPTION, LEFT_OPTION);
    assert!(events.is_empty(), "the other side does not press");
}

#[test]
fn a_key_typed_with_the_modifier_interrupts_and_nothing_is_kept() {
    let mut keys = right_option();
    keys.modifier(vk::RIGHT_OPTION, RIGHT_OPTION);
    let (events, disposition) = keys.down(KEY_E, RIGHT_OPTION);
    assert_eq!(states(&events), one("talk", ShortcutState::Interrupted));
    assert_eq!(disposition, Disposition::PassThrough);
    assert_eq!(
        keys.up(KEY_E, RIGHT_OPTION),
        (vec![], Disposition::PassThrough)
    );
    let (events, _) = keys.modifier(vk::RIGHT_OPTION, 0);
    assert!(events.is_empty(), "no release after an interruption");
}

#[test]
fn another_modifier_joining_interrupts() {
    // Another modifier, or the other side of the same one.
    let both = flags::OPTION | flags::LEFT_OPTION | flags::RIGHT_OPTION;
    for (key, flags) in [(vk::SHIFT, RIGHT_OPTION | LEFT_SHIFT), (vk::OPTION, both)] {
        let mut keys = right_option();
        keys.modifier(vk::RIGHT_OPTION, RIGHT_OPTION);
        let (events, _) = keys.modifier(key, flags);
        assert_eq!(states(&events), one("talk", ShortcutState::Interrupted));
    }
}

#[test]
fn the_modifier_pressed_while_something_else_is_held_does_not_press() {
    let mut keys = right_option();
    keys.modifier(vk::SHIFT, LEFT_SHIFT);
    let (events, _) = keys.modifier(vk::RIGHT_OPTION, LEFT_SHIFT | RIGHT_OPTION);
    assert!(events.is_empty(), "Shift is held");
    keys.modifier(vk::RIGHT_OPTION, LEFT_SHIFT);
    keys.modifier(vk::SHIFT, 0);

    keys.down(KEY_A, 0);
    let (events, _) = keys.modifier(vk::RIGHT_OPTION, RIGHT_OPTION);
    assert!(events.is_empty(), "a key is held");
}

#[test]
fn fn_on_its_own_presses_and_releases() {
    let mut keys = Keys::with(vec![lone("talk", ModifierKey::Fn)]);
    let (events, _) = keys.modifier(vk::FUNCTION, flags::FN);
    assert_eq!(states(&events), one("talk", ShortcutState::Pressed));
    let (events, _) = keys.modifier(vk::FUNCTION, 0);
    assert_eq!(states(&events), one("talk", ShortcutState::Released));
}

#[test]
fn a_lone_modifier_and_a_combination_on_it_both_work() {
    let mut keys = Keys::with(vec![
        lone("lone", ModifierKey::RightAlt),
        combo("combo", &[ModifierKey::RightAlt], KeyCode::Space),
    ]);
    keys.modifier(vk::RIGHT_OPTION, RIGHT_OPTION);
    let (events, disposition) = keys.down(SPACE, RIGHT_OPTION);
    assert_eq!(
        states(&events),
        vec![
            ("lone".to_owned(), ShortcutState::Interrupted),
            ("combo".to_owned(), ShortcutState::Pressed)
        ]
    );
    assert_eq!(disposition, Disposition::Swallow);
}

#[test]
fn caps_lock_reports_each_press_as_down_and_straight_up() {
    let mut keys = Keys::with(vec![lone_toggle("toggle", ModifierKey::CapsLock)]);
    let tap = vec![
        ("toggle".to_owned(), ShortcutState::Pressed),
        ("toggle".to_owned(), ShortcutState::Released),
    ];
    let (events, disposition) = keys.modifier(vk::CAPS_LOCK, flags::CAPS_LOCK);
    assert_eq!(states(&events), tap);
    assert_eq!(disposition, Disposition::PassThrough);
    let (events, _) = keys.modifier(vk::CAPS_LOCK, 0);
    assert_eq!(states(&events), tap, "the press that switches the lock off");
}

#[test]
fn caps_lock_reported_again_without_a_change_is_not_another_press() {
    let mut keys = Keys::with(vec![lone_toggle("toggle", ModifierKey::CapsLock)]);
    keys.modifier(vk::CAPS_LOCK, flags::CAPS_LOCK);
    let (events, _) = keys.modifier(vk::CAPS_LOCK, flags::CAPS_LOCK);
    assert!(events.is_empty());
}

#[test]
fn caps_lock_switched_while_not_watched_is_noticed_from_other_events() {
    let mut keys = Keys::with(vec![lone_toggle("toggle", ModifierKey::CapsLock)]);
    keys.down(KEY_A, flags::CAPS_LOCK);
    keys.up(KEY_A, flags::CAPS_LOCK);
    let (events, _) = keys.modifier(vk::CAPS_LOCK, 0);
    assert_eq!(states(&events).len(), 2, "the lock went off: a press");
}

#[test]
fn a_mouse_side_button_presses_releases_and_is_kept_from_the_application() {
    let mut keys = Keys::with(vec![lone("talk", ModifierKey::MouseSideBack)]);
    let (events, disposition) = keys.mouse_down(3);
    assert_eq!(states(&events), one("talk", ShortcutState::Pressed));
    assert_eq!(disposition, Disposition::Swallow);
    let (events, disposition) = keys.mouse_up(3);
    assert_eq!(states(&events), one("talk", ShortcutState::Released));
    assert_eq!(disposition, Disposition::Swallow);
}

#[test]
fn a_mouse_button_that_is_not_a_shortcut_goes_through() {
    let mut keys = Keys::with(vec![lone("talk", ModifierKey::MouseSideBack)]);
    assert_eq!(keys.mouse_down(4), (vec![], Disposition::PassThrough));
    assert_eq!(keys.mouse_up(4), (vec![], Disposition::PassThrough));
    assert_eq!(keys.mouse_down(2), (vec![], Disposition::PassThrough));
}

#[test]
fn a_key_pressed_while_a_mouse_button_is_held_interrupts() {
    let mut keys = Keys::with(vec![lone("talk", ModifierKey::MouseSideForward)]);
    keys.mouse_down(4);
    let (events, _) = keys.down(KEY_A, 0);
    assert_eq!(states(&events), one("talk", ShortcutState::Interrupted));
    let (events, disposition) = keys.mouse_up(4);
    assert!(events.is_empty());
    assert_eq!(
        disposition,
        Disposition::Swallow,
        "its press was kept, so its release is too"
    );
}

#[test]
fn interrupting_after_missed_events_ends_the_press_once() {
    let mut keys = right_option();
    keys.modifier(vk::RIGHT_OPTION, RIGHT_OPTION);
    let event = keys
        .matcher
        .interrupt(5_000)
        .expect("a press was in progress");
    assert_eq!(event.state, ShortcutState::Interrupted);
    assert_eq!(event.timestamp_ms, 5_000);
    assert!(keys.matcher.interrupt(6_000).is_none());
}

#[test]
fn a_click_scroll_or_drag_with_a_lone_modifier_down_interrupts_it_and_goes_through() {
    // Shift-click, Command-click, Control-click, Option-click, a middle click, Control-scroll
    // and a drag.
    for (modifier, key, held) in [
        (ModifierKey::LeftShift, vk::SHIFT, LEFT_SHIFT),
        (ModifierKey::LeftMeta, vk::COMMAND, LEFT_COMMAND),
        (ModifierKey::LeftControl, vk::CONTROL, LEFT_CONTROL),
        (ModifierKey::LeftAlt, vk::OPTION, LEFT_OPTION),
    ] {
        for input in [
            InputKind::MouseDown { button: 0 },
            InputKind::MouseDown { button: 1 },
            InputKind::MouseDown { button: 2 },
            InputKind::Scroll,
            InputKind::MouseDragged { button: 0 },
        ] {
            let mut keys = Keys::with(vec![lone("talk", modifier)]);
            keys.modifier(key, held);
            let (events, disposition) = keys.send(input, held);
            assert_eq!(
                states(&events),
                one("talk", ShortcutState::Interrupted),
                "{modifier:?} with {input:?}"
            );
            assert_eq!(disposition, Disposition::PassThrough);
            let (events, _) = keys.modifier(key, 0);
            assert!(events.is_empty(), "no release after an interruption");
        }
    }
}

#[test]
fn clicks_and_scrolling_leave_a_combination_and_no_press_alone() {
    let mut keys = Keys::with(vec![combo(
        "talk",
        &[ModifierKey::LeftControl],
        KeyCode::Space,
    )]);
    assert_eq!(
        keys.send(InputKind::MouseDown { button: 0 }, 0),
        (vec![], Disposition::PassThrough)
    );
    assert_eq!(
        keys.send(InputKind::Scroll, 0),
        (vec![], Disposition::PassThrough)
    );
    keys.modifier(vk::CONTROL, LEFT_CONTROL);
    keys.down(SPACE, LEFT_CONTROL);
    let (events, disposition) = keys.send(InputKind::Scroll, LEFT_CONTROL);
    assert!(events.is_empty());
    assert_eq!(disposition, Disposition::PassThrough);
    let (events, _) = keys.up(SPACE, LEFT_CONTROL);
    assert_eq!(states(&events), one("talk", ShortcutState::Released));
}

#[test]
fn a_shortcut_key_still_held_after_its_combination_ended_blocks_a_lone_modifier() {
    let mut keys = Keys::with(vec![
        combo("combo", &[ModifierKey::LeftControl], KeyCode::Space),
        lone("lone", ModifierKey::RightAlt),
    ]);
    keys.modifier(vk::CONTROL, LEFT_CONTROL);
    keys.down(SPACE, LEFT_CONTROL);
    let (events, _) = keys.modifier(vk::CONTROL, 0);
    assert_eq!(states(&events), one("combo", ShortcutState::Released));
    // Space is still held.
    let (events, _) = keys.modifier(vk::RIGHT_OPTION, RIGHT_OPTION);
    assert!(events.is_empty(), "Space is still down");
    keys.modifier(vk::RIGHT_OPTION, 0);
    assert_eq!(keys.up(SPACE, 0), (vec![], Disposition::Swallow));
    let (events, _) = keys.modifier(vk::RIGHT_OPTION, RIGHT_OPTION);
    assert_eq!(states(&events), one("lone", ShortcutState::Pressed));
}

#[test]
fn a_lone_modifier_pressed_during_a_drag_does_not_go_down() {
    for button in [0, 1] {
        let mut keys = right_option();
        assert_eq!(keys.mouse_down(button), (vec![], Disposition::PassThrough));
        let drag = keys.send(InputKind::MouseDragged { button }, 0);
        assert_eq!(drag, (vec![], Disposition::PassThrough));
        let (events, _) = keys.modifier(vk::RIGHT_OPTION, RIGHT_OPTION);
        assert!(events.is_empty(), "button {button} is held");
        let drop = keys.send(InputKind::MouseUp { button }, RIGHT_OPTION);
        assert_eq!(drop, (vec![], Disposition::PassThrough));
        let (events, _) = keys.modifier(vk::RIGHT_OPTION, 0);
        assert!(events.is_empty(), "no release for a press never reported");
        let (events, _) = keys.modifier(vk::RIGHT_OPTION, RIGHT_OPTION);
        assert_eq!(states(&events), one("talk", ShortcutState::Pressed));
    }
}

#[test]
fn a_drag_whose_press_was_not_seen_still_keeps_a_lone_modifier_up() {
    let mut keys = right_option();
    keys.send(InputKind::MouseDragged { button: 0 }, 0);
    let (events, _) = keys.modifier(vk::RIGHT_OPTION, RIGHT_OPTION);
    assert!(events.is_empty());
}
