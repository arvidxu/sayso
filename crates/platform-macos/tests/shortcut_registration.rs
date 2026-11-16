mod support;

use dictation_types::{
    ErrorKind, HotkeyBinding, HotkeyTriggerMode, KeyCode, ModifierKey, ShortcutState,
};
use platform_macos::ShortcutMatcher;
use platform_macos::keys::vk;
use support::*;

fn refused(binding: HotkeyBinding) -> ErrorKind {
    ShortcutMatcher::new()
        .register(binding)
        .expect_err("must be refused")
        .kind
}

#[test]
fn caps_lock_cannot_be_held() {
    assert_eq!(
        refused(lone("talk", ModifierKey::CapsLock)),
        ErrorKind::Validation
    );
    let mut matcher = ShortcutMatcher::new();
    matcher
        .register(lone_toggle("talk", ModifierKey::CapsLock))
        .unwrap();
}

#[test]
fn caps_lock_and_mouse_buttons_work_only_on_their_own() {
    for modifier in [
        ModifierKey::CapsLock,
        ModifierKey::MouseSideBack,
        ModifierKey::MouseSideForward,
    ] {
        let binding = combo(
            "talk",
            &[ModifierKey::LeftControl, modifier],
            KeyCode::Space,
        );
        assert_eq!(refused(binding), ErrorKind::Validation, "{modifier:?}");
    }
}

#[test]
fn keys_macos_does_not_have_are_refused() {
    let control = &[ModifierKey::LeftControl];
    for key in [
        KeyCode::Function(21),
        KeyCode::Function(24),
        KeyCode::Character('é'),
        KeyCode::Character('ß'),
        KeyCode::Named("Insert".to_owned()),
        KeyCode::Named("arrowup".to_owned()),
    ] {
        assert_eq!(
            refused(combo("talk", control, key.clone())),
            ErrorKind::Validation,
            "{key:?}"
        );
    }
    // A binding that fails its own check: a letter without a modifier.
    assert_eq!(
        refused(combo("talk", &[], KeyCode::Character('a'))),
        ErrorKind::Validation
    );
}

#[test]
fn two_shortcuts_on_the_same_keys_are_refused() {
    let mut matcher = ShortcutMatcher::new();
    matcher
        .register(lone("first", ModifierKey::RightAlt))
        .unwrap();
    let err = matcher
        .register(lone("second", ModifierKey::RightAlt))
        .unwrap_err();
    assert_eq!(err.kind, ErrorKind::AlreadyExists);
    assert!(err.message.contains("first"));
    // How they trigger does not make them different keys either.
    let err = matcher
        .register(lone_toggle("toggle", ModifierKey::RightAlt))
        .unwrap_err();
    assert_eq!(err.kind, ErrorKind::AlreadyExists);
    // The order the modifiers are listed in does not make them different keys.
    let control_option = [ModifierKey::LeftControl, ModifierKey::LeftAlt];
    matcher
        .register(combo("a", &control_option, KeyCode::Space))
        .unwrap();
    let reversed = [ModifierKey::LeftAlt, ModifierKey::LeftControl];
    let err = matcher
        .register(combo("b", &reversed, KeyCode::Space))
        .unwrap_err();
    assert_eq!(err.kind, ErrorKind::AlreadyExists);
}

#[test]
fn registering_the_same_id_again_replaces_it() {
    let mut matcher = ShortcutMatcher::new();
    matcher
        .register(lone("talk", ModifierKey::RightAlt))
        .unwrap();
    matcher
        .register(lone_toggle("talk", ModifierKey::Fn))
        .unwrap();
    let bindings = matcher.bindings();
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].modifiers, [ModifierKey::Fn]);
    assert_eq!(bindings[0].trigger_mode, HotkeyTriggerMode::ToggleOnPress);
}

#[test]
fn a_refused_replacement_keeps_the_old_binding() {
    let mut matcher = ShortcutMatcher::new();
    matcher
        .register(lone("talk", ModifierKey::RightAlt))
        .unwrap();
    matcher
        .register(lone("caps", ModifierKey::CapsLock))
        .unwrap_err();
    matcher
        .register(lone("talk", ModifierKey::CapsLock))
        .unwrap_err();
    assert_eq!(matcher.bindings()[0].modifiers, [ModifierKey::RightAlt]);
}

#[test]
fn unregistering_reports_whether_it_existed_and_queues_an_interruption_for_a_press() {
    let mut keys = Keys::with(vec![lone("talk", ModifierKey::RightAlt)]);
    keys.modifier(vk::RIGHT_OPTION, RIGHT_OPTION);
    assert!(keys.matcher.unregister(&id("talk")));
    assert!(!keys.matcher.unregister(&id("talk")));
    assert_eq!(
        states(&keys.matcher.take_pending()),
        one("talk", ShortcutState::Interrupted)
    );
    let (events, _) = keys.modifier(vk::RIGHT_OPTION, 0);
    assert!(events.is_empty(), "no release after the interruption");

    // Clearing every binding during a press interrupts it the same way.
    let mut keys = Keys::with(vec![lone("talk", ModifierKey::RightAlt)]);
    keys.modifier(vk::RIGHT_OPTION, RIGHT_OPTION);
    keys.matcher.clear();
    assert_eq!(
        states(&keys.matcher.take_pending()),
        one("talk", ShortcutState::Interrupted)
    );
}

#[test]
fn a_key_reached_through_fn_is_the_same_keys_with_and_without_fn() {
    let control = ModifierKey::LeftControl;
    let pairs = [
        (vec![], vec![ModifierKey::Fn], KeyCode::Function(5)),
        (
            vec![control],
            vec![control, ModifierKey::Fn],
            KeyCode::Named("PageUp".to_owned()),
        ),
        (
            vec![control],
            vec![ModifierKey::Fn, control],
            KeyCode::Named("Home".to_owned()),
        ),
        (
            vec![control],
            vec![control, ModifierKey::Fn],
            KeyCode::Named("Delete".to_owned()),
        ),
    ];
    for (without, with, key) in pairs {
        for (first, second) in [(&without, &with), (&with, &without)] {
            let mut matcher = ShortcutMatcher::new();
            matcher
                .register(combo("first", first, key.clone()))
                .unwrap();
            let err = matcher
                .register(combo("second", second, key.clone()))
                .unwrap_err();
            assert_eq!(
                err.kind,
                ErrorKind::AlreadyExists,
                "{first:?} {second:?} {key:?}"
            );
        }
    }
    // Fn still tells apart keys that are not reached through it.
    let mut matcher = ShortcutMatcher::new();
    matcher
        .register(combo("first", &[control], KeyCode::Space))
        .unwrap();
    matcher
        .register(combo("second", &[control, ModifierKey::Fn], KeyCode::Space))
        .unwrap();
}

#[test]
fn fn_with_a_key_macos_turns_into_another_is_refused() {
    let control = ModifierKey::LeftControl;
    for key in [
        KeyCode::Named("Backspace".to_owned()),
        KeyCode::Named("ArrowLeft".to_owned()),
        KeyCode::Named("ArrowRight".to_owned()),
        KeyCode::Named("ArrowUp".to_owned()),
        KeyCode::Named("ArrowDown".to_owned()),
        KeyCode::Return,
    ] {
        for modifiers in [vec![ModifierKey::Fn], vec![control, ModifierKey::Fn]] {
            let err = ShortcutMatcher::new()
                .register(combo("talk", &modifiers, key.clone()))
                .expect_err("must be refused");
            assert_eq!(err.kind, ErrorKind::Validation, "{modifiers:?} {key:?}");
            assert!(err.message.contains("Fn"), "{err}");
        }
        // Without Fn the same key is fine.
        ShortcutMatcher::new()
            .register(combo("talk", &[control], key.clone()))
            .unwrap();
    }
    // Fn with keys it leaves alone still works.
    for key in [
        KeyCode::Space,
        KeyCode::Named("Delete".to_owned()),
        KeyCode::Named("Home".to_owned()),
    ] {
        ShortcutMatcher::new()
            .register(combo("talk", &[control, ModifierKey::Fn], key.clone()))
            .unwrap();
    }
}

#[test]
fn double_tap_on_a_mouse_side_button_is_refused() {
    for button in [ModifierKey::MouseSideBack, ModifierKey::MouseSideForward] {
        let binding = HotkeyBinding::standalone_modifier(
            id("talk"),
            button,
            HotkeyTriggerMode::DoubleTapToggle,
        );
        assert_eq!(refused(binding), ErrorKind::Validation, "{button:?}");
    }
    let mut matcher = ShortcutMatcher::new();
    matcher
        .register(HotkeyBinding::standalone_modifier(
            id("talk"),
            ModifierKey::RightAlt,
            HotkeyTriggerMode::DoubleTapToggle,
        ))
        .unwrap();
}

#[test]
fn a_removed_shortcut_still_keeps_the_release_of_a_key_it_kept() {
    let mut keys = Keys::with(vec![combo(
        "talk",
        &[ModifierKey::LeftControl],
        KeyCode::Space,
    )]);
    keys.modifier(vk::CONTROL, LEFT_CONTROL);
    keys.down(SPACE, LEFT_CONTROL);
    keys.matcher.clear();
    assert!(keys.matcher.bindings().is_empty());
    let (events, disposition) = keys.up(SPACE, LEFT_CONTROL);
    assert!(events.is_empty());
    assert_eq!(disposition, platform_macos::Disposition::Swallow);
}

#[test]
fn every_character_key_maps_to_its_own_place() {
    let characters = r"abcdefghijklmnopqrstuvwxyz0123456789-=[]\;',./`";
    let mut seen = std::collections::BTreeSet::new();
    for ch in characters.chars() {
        let key = platform_macos::keys::virtual_key(&KeyCode::Character(ch))
            .unwrap_or_else(|| panic!("{ch} has a key"));
        assert!(seen.insert(key), "{ch} shares a key");
    }
    assert_eq!(
        platform_macos::keys::virtual_key(&KeyCode::Character('A')),
        platform_macos::keys::virtual_key(&KeyCode::Character('a'))
    );
}
