mod support;

use dictation_types::{KeyCode, ModifierKey, ShortcutState};
use platform_macos::Disposition;
use platform_macos::keys::{flags, vk};
use support::*;

fn control_option_space() -> Keys {
    Keys::with(vec![combo(
        "talk",
        &[ModifierKey::LeftControl, ModifierKey::LeftAlt],
        KeyCode::Space,
    )])
}

const CONTROL_OPTION: u64 = LEFT_CONTROL | LEFT_OPTION;

#[test]
fn the_key_with_its_modifiers_presses_and_the_key_coming_up_releases() {
    let mut keys = control_option_space();
    keys.modifier(vk::CONTROL, LEFT_CONTROL);
    keys.modifier(vk::OPTION, CONTROL_OPTION);
    let (events, disposition) = keys.down(SPACE, CONTROL_OPTION);
    assert_eq!(states(&events), one("talk", ShortcutState::Pressed));
    assert_eq!(disposition, Disposition::Swallow);
    let (events, disposition) = keys.up(SPACE, CONTROL_OPTION);
    assert_eq!(states(&events), one("talk", ShortcutState::Released));
    assert_eq!(disposition, Disposition::Swallow);
}

#[test]
fn repeats_while_held_are_kept_from_the_application_and_report_nothing() {
    let mut keys = control_option_space();
    keys.modifier(vk::CONTROL, LEFT_CONTROL);
    keys.modifier(vk::OPTION, CONTROL_OPTION);
    keys.down(SPACE, CONTROL_OPTION);
    for _ in 0..3 {
        let (events, disposition) = keys.repeat(SPACE, CONTROL_OPTION);
        assert!(events.is_empty());
        assert_eq!(disposition, Disposition::Swallow);
    }
}

#[test]
fn letting_go_of_a_modifier_first_releases_and_the_key_coming_up_is_still_kept() {
    let mut keys = control_option_space();
    keys.modifier(vk::CONTROL, LEFT_CONTROL);
    keys.modifier(vk::OPTION, CONTROL_OPTION);
    keys.down(SPACE, CONTROL_OPTION);
    let (events, _) = keys.modifier(vk::OPTION, LEFT_CONTROL);
    assert_eq!(states(&events), one("talk", ShortcutState::Released));
    let (events, disposition) = keys.up(SPACE, LEFT_CONTROL);
    assert!(events.is_empty());
    assert_eq!(disposition, Disposition::Swallow);
}

#[test]
fn an_extra_modifier_or_a_missing_one_does_not_match_and_the_key_goes_through() {
    let mut keys = control_option_space();
    keys.modifier(vk::CONTROL, LEFT_CONTROL);
    let (events, disposition) = keys.down(SPACE, LEFT_CONTROL);
    assert!(events.is_empty());
    assert_eq!(disposition, Disposition::PassThrough);
    keys.up(SPACE, LEFT_CONTROL);

    keys.modifier(vk::OPTION, CONTROL_OPTION);
    keys.modifier(vk::COMMAND, CONTROL_OPTION | LEFT_COMMAND);
    let (events, disposition) = keys.down(SPACE, CONTROL_OPTION | LEFT_COMMAND);
    assert!(events.is_empty());
    assert_eq!(disposition, Disposition::PassThrough);
}

#[test]
fn the_side_of_a_modifier_counts() {
    let mut keys = control_option_space();
    keys.modifier(vk::RIGHT_CONTROL, RIGHT_CONTROL);
    keys.modifier(vk::OPTION, RIGHT_CONTROL | LEFT_OPTION);
    let (events, disposition) = keys.down(SPACE, RIGHT_CONTROL | LEFT_OPTION);
    assert!(events.is_empty(), "right Control is not left Control");
    assert_eq!(disposition, Disposition::PassThrough);
}

#[test]
fn modifiers_from_a_program_without_side_flags_take_their_side_from_the_key() {
    let mut keys = control_option_space();
    keys.modifier(vk::CONTROL, flags::CONTROL);
    keys.modifier(vk::OPTION, flags::CONTROL | flags::OPTION);
    let (events, _) = keys.down(SPACE, flags::CONTROL | flags::OPTION);
    assert_eq!(states(&events), one("talk", ShortcutState::Pressed));
}

#[test]
fn another_key_while_down_interrupts_and_goes_through() {
    let mut keys = control_option_space();
    keys.modifier(vk::CONTROL, LEFT_CONTROL);
    keys.modifier(vk::OPTION, CONTROL_OPTION);
    keys.down(SPACE, CONTROL_OPTION);
    let (events, disposition) = keys.down(KEY_A, CONTROL_OPTION);
    assert_eq!(states(&events), one("talk", ShortcutState::Interrupted));
    assert_eq!(disposition, Disposition::PassThrough);
    let (events, disposition) = keys.up(SPACE, CONTROL_OPTION);
    assert!(events.is_empty(), "no release after an interruption");
    assert_eq!(disposition, Disposition::Swallow);

    // Another modifier joining interrupts too.
    let mut keys = control_option_space();
    keys.modifier(vk::CONTROL, LEFT_CONTROL);
    keys.modifier(vk::OPTION, CONTROL_OPTION);
    keys.down(SPACE, CONTROL_OPTION);
    let (events, _) = keys.modifier(vk::SHIFT, CONTROL_OPTION | LEFT_SHIFT);
    assert_eq!(states(&events), one("talk", ShortcutState::Interrupted));
}

#[test]
fn keys_that_are_not_shortcuts_always_go_through() {
    let mut keys = control_option_space();
    for (key, flags) in [(KEY_A, 0), (SPACE, 0), (KEY_E, LEFT_SHIFT)] {
        assert_eq!(keys.down(key, flags), (vec![], Disposition::PassThrough));
        assert_eq!(keys.repeat(key, flags), (vec![], Disposition::PassThrough));
        assert_eq!(keys.up(key, flags), (vec![], Disposition::PassThrough));
    }
}

#[test]
fn a_function_key_works_without_modifiers() {
    let mut keys = Keys::with(vec![combo("talk", &[], KeyCode::Function(13))]);
    let f13 = 0x69;
    let (events, disposition) = keys.down(f13, flags::FN);
    assert_eq!(states(&events), one("talk", ShortcutState::Pressed));
    assert_eq!(disposition, Disposition::Swallow);
    let (events, _) = keys.up(f13, flags::FN);
    assert_eq!(states(&events), one("talk", ShortcutState::Released));
}

#[test]
fn the_fn_flag_the_system_puts_on_navigation_keys_is_not_taken_for_fn_held() {
    let mut keys = Keys::with(vec![combo(
        "talk",
        &[ModifierKey::LeftControl],
        KeyCode::Named("ArrowUp".to_owned()),
    )]);
    keys.modifier(vk::CONTROL, LEFT_CONTROL);
    let (events, _) = keys.down(vk::UP_ARROW, LEFT_CONTROL | flags::FN);
    assert_eq!(states(&events), one("talk", ShortcutState::Pressed));
}

#[test]
fn fn_is_held_only_after_the_fn_key_went_down() {
    let mut keys = Keys::with(vec![combo("talk", &[ModifierKey::Fn], KeyCode::Space)]);
    let (events, _) = keys.down(SPACE, flags::FN);
    assert!(events.is_empty(), "the flag alone does not mean Fn is held");
    keys.up(SPACE, 0);
    keys.modifier(vk::FUNCTION, flags::FN);
    let (events, _) = keys.down(SPACE, flags::FN);
    assert_eq!(states(&events), one("talk", ShortcutState::Pressed));
    let (events, _) = keys.modifier(vk::FUNCTION, 0);
    assert_eq!(states(&events), one("talk", ShortcutState::Released));
}

#[test]
fn a_fresh_press_after_a_missed_release_is_a_new_press() {
    let mut keys = control_option_space();
    keys.modifier(vk::CONTROL, LEFT_CONTROL);
    keys.modifier(vk::OPTION, CONTROL_OPTION);
    keys.down(SPACE, CONTROL_OPTION);
    keys.modifier(vk::OPTION, LEFT_CONTROL);
    keys.modifier(vk::OPTION, CONTROL_OPTION);
    // The release of Space was never seen.
    let (events, disposition) = keys.down(SPACE, CONTROL_OPTION);
    assert_eq!(states(&events), one("talk", ShortcutState::Pressed));
    assert_eq!(disposition, Disposition::Swallow);
}
