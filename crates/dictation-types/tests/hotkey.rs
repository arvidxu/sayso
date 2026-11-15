use dictation_types::{
    ErrorKind, HotkeyBinding, HotkeyTriggerMode, KeyCode, ModeId, ModifierKey, ShortcutId,
};

fn binding(modifiers: Vec<ModifierKey>, key: Option<KeyCode>) -> HotkeyBinding {
    HotkeyBinding {
        id: ShortcutId::new("shortcut-1"),
        modifiers,
        key,
        trigger_mode: HotkeyTriggerMode::HoldToTalk,
        mode_id: None,
    }
}

#[test]
fn a_binding_with_no_key_and_no_modifier_is_invalid() {
    let err = binding(Vec::new(), None).validate().unwrap_err();
    assert_eq!(err.kind, ErrorKind::Validation);
}

#[test]
fn a_binding_with_a_repeated_modifier_is_invalid() {
    let with_key = binding(
        vec![ModifierKey::LeftControl, ModifierKey::LeftControl],
        Some(KeyCode::Space),
    );
    assert!(with_key.validate().is_err());

    let spread = binding(
        vec![
            ModifierKey::LeftAlt,
            ModifierKey::LeftShift,
            ModifierKey::LeftAlt,
        ],
        Some(KeyCode::Character('d')),
    );
    assert!(spread.validate().is_err());

    let without_key = binding(vec![ModifierKey::Fn, ModifierKey::Fn], None);
    assert!(without_key.validate().is_err());
}

#[test]
fn several_modifiers_without_a_main_key_are_invalid() {
    assert!(
        binding(vec![ModifierKey::LeftControl, ModifierKey::LeftAlt], None)
            .validate()
            .is_err()
    );
    assert!(
        binding(
            vec![
                ModifierKey::LeftMeta,
                ModifierKey::LeftShift,
                ModifierKey::RightAlt
            ],
            None
        )
        .validate()
        .is_err()
    );
}

#[test]
fn a_lone_modifier_is_a_valid_binding() {
    for modifier in [
        ModifierKey::Fn,
        ModifierKey::CapsLock,
        ModifierKey::RightAlt,
        ModifierKey::RightMeta,
        ModifierKey::LeftControl,
        ModifierKey::MouseSideBack,
    ] {
        for trigger in [
            HotkeyTriggerMode::HoldToTalk,
            HotkeyTriggerMode::ToggleOnPress,
            HotkeyTriggerMode::DoubleTapToggle,
        ] {
            let lone = HotkeyBinding::standalone_modifier(ShortcutId::new("s"), modifier, trigger);
            assert!(lone.validate().is_ok(), "{modifier:?} {trigger:?}");
            assert!(lone.is_standalone_modifier());
            assert_eq!(lone.key, None);
            assert_eq!(lone.mode_id, None);
        }
    }
}

#[test]
fn a_modifier_with_a_key_is_a_valid_binding() {
    let combos = [
        (vec![ModifierKey::LeftControl], KeyCode::Space),
        (
            vec![ModifierKey::LeftMeta, ModifierKey::LeftShift],
            KeyCode::Character('d'),
        ),
        (
            vec![
                ModifierKey::LeftControl,
                ModifierKey::LeftAlt,
                ModifierKey::LeftShift,
            ],
            KeyCode::Function(9),
        ),
        (
            vec![ModifierKey::RightAlt],
            KeyCode::Named("Insert".to_owned()),
        ),
        (
            vec![ModifierKey::LeftControl, ModifierKey::RightControl],
            KeyCode::Return,
        ),
    ];
    for (modifiers, key) in combos {
        let combo = HotkeyBinding::combination(
            ShortcutId::new("s"),
            modifiers.clone(),
            key.clone(),
            HotkeyTriggerMode::ToggleOnPress,
        );
        assert!(combo.validate().is_ok(), "{modifiers:?} + {key:?}");
        assert!(!combo.is_standalone_modifier());
        assert_eq!(combo.key, Some(key));
    }
}

#[test]
fn a_binding_without_an_id_is_invalid() {
    let unnamed = HotkeyBinding::standalone_modifier(
        ShortcutId::new("  "),
        ModifierKey::Fn,
        HotkeyTriggerMode::HoldToTalk,
    );
    assert!(unnamed.validate().is_err());
}

#[test]
fn a_binding_starts_the_active_mode_unless_it_names_one() {
    let lone = HotkeyBinding::standalone_modifier(
        ShortcutId::new("s"),
        ModifierKey::Fn,
        HotkeyTriggerMode::HoldToTalk,
    );
    assert_eq!(lone.mode_id, None);
    let named = lone.with_mode(ModeId::new("mode-x"));
    assert_eq!(named.mode_id, Some(ModeId::new("mode-x")));
    assert!(named.validate().is_ok());
}

#[test]
fn shift_with_an_editing_or_navigation_key_is_invalid_as_it_would_catch_text_selection() {
    let typing_only = [
        vec![ModifierKey::LeftShift],
        vec![ModifierKey::RightShift],
        vec![ModifierKey::CapsLock],
        vec![ModifierKey::LeftShift, ModifierKey::MouseSideBack],
    ];
    for name in [
        "ArrowLeft",
        "ArrowRight",
        "ArrowUp",
        "ArrowDown",
        "Home",
        "End",
        "PageUp",
        "PageDown",
        "Backspace",
        "Delete",
        "Insert",
    ] {
        for modifiers in &typing_only {
            let shortcut = binding(modifiers.clone(), Some(KeyCode::Named(name.to_owned())));
            let err = shortcut.validate().unwrap_err();
            assert_eq!(err.kind, ErrorKind::Validation, "{modifiers:?} + {name}");
        }
        let with_control = binding(
            vec![ModifierKey::LeftControl, ModifierKey::LeftShift],
            Some(KeyCode::Named(name.to_owned())),
        );
        assert!(with_control.validate().is_ok(), "Control+Shift+{name}");
    }
    let other_named = binding(
        vec![ModifierKey::LeftShift],
        Some(KeyCode::Named("Pause".to_owned())),
    );
    assert!(
        other_named.validate().is_ok(),
        "a key that does not edit or move"
    );
}

#[test]
fn a_named_key_must_be_a_known_name_in_its_own_case() {
    // Each of these would slip past the typing-key rule if accepted.
    for name in [
        "Tab",
        "Enter",
        " ",
        "Escape",
        "arrowleft",
        "ARROWLEFT",
        "home",
        "F1",
        "Nope",
    ] {
        let shift = binding(
            vec![ModifierKey::LeftShift],
            Some(KeyCode::Named(name.to_owned())),
        );
        assert_eq!(
            shift.validate().unwrap_err().kind,
            ErrorKind::Validation,
            "{name:?}"
        );
        let control = binding(
            vec![ModifierKey::LeftControl],
            Some(KeyCode::Named(name.to_owned())),
        );
        assert_eq!(
            control.validate().unwrap_err().kind,
            ErrorKind::Validation,
            "{name:?}"
        );
    }
    for name in [
        "Backspace",
        "Delete",
        "Insert",
        "Home",
        "End",
        "PageUp",
        "PageDown",
        "ArrowLeft",
        "ArrowRight",
        "ArrowUp",
        "ArrowDown",
        "PrintScreen",
        "ScrollLock",
        "Pause",
        "ContextMenu",
        "Help",
    ] {
        let control = binding(
            vec![ModifierKey::LeftControl],
            Some(KeyCode::Named((*name).to_owned())),
        );
        assert!(control.validate().is_ok(), "Control+{name}");
    }
}
