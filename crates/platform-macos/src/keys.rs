use dictation_types::{KeyCode, ModifierKey};

pub use crate::keys_data::{VirtualKey, flags, vk};
use crate::keys_data::{character_key, function_key, is_function_key, mouse};

/// Each modifier that has a side, with the flag both sides share, the flag of its own side and
/// the flag of the other side.
#[rustfmt::skip]
pub(crate) const SIDED_MODIFIERS: &[(ModifierKey, u64, u64, u64)] = &[
    (ModifierKey::LeftShift, flags::SHIFT, flags::LEFT_SHIFT, flags::RIGHT_SHIFT),
    (ModifierKey::RightShift, flags::SHIFT, flags::RIGHT_SHIFT, flags::LEFT_SHIFT),
    (ModifierKey::LeftControl, flags::CONTROL, flags::LEFT_CONTROL, flags::RIGHT_CONTROL),
    (ModifierKey::RightControl, flags::CONTROL, flags::RIGHT_CONTROL, flags::LEFT_CONTROL),
    (ModifierKey::LeftAlt, flags::OPTION, flags::LEFT_OPTION, flags::RIGHT_OPTION),
    (ModifierKey::RightAlt, flags::OPTION, flags::RIGHT_OPTION, flags::LEFT_OPTION),
    (ModifierKey::LeftMeta, flags::COMMAND, flags::LEFT_COMMAND, flags::RIGHT_COMMAND),
    (ModifierKey::RightMeta, flags::COMMAND, flags::RIGHT_COMMAND, flags::LEFT_COMMAND),
];

/// Caps Lock is included.
pub(crate) fn modifier_of_key(key: VirtualKey) -> Option<ModifierKey> {
    Some(match key {
        vk::FUNCTION => ModifierKey::Fn,
        vk::CAPS_LOCK => ModifierKey::CapsLock,
        vk::SHIFT => ModifierKey::LeftShift,
        vk::RIGHT_SHIFT => ModifierKey::RightShift,
        vk::CONTROL => ModifierKey::LeftControl,
        vk::RIGHT_CONTROL => ModifierKey::RightControl,
        vk::OPTION => ModifierKey::LeftAlt,
        vk::RIGHT_OPTION => ModifierKey::RightAlt,
        vk::COMMAND => ModifierKey::LeftMeta,
        vk::RIGHT_COMMAND => ModifierKey::RightMeta,
        _ => return None,
    })
}

/// The flag both sides share, the flag of its own side and the flag of the other side. `None`
/// for Fn, Caps Lock and the mouse buttons.
pub(crate) fn sided_flags(modifier: ModifierKey) -> Option<(u64, u64, u64)> {
    SIDED_MODIFIERS
        .iter()
        .find(|(sided, ..)| *sided == modifier)
        .map(|&(_, shared, own, other)| (shared, own, other))
}

pub(crate) fn mouse_button(modifier: ModifierKey) -> Option<i64> {
    match modifier {
        ModifierKey::MouseSideBack => Some(mouse::SIDE_BACK),
        ModifierKey::MouseSideForward => Some(mouse::SIDE_FORWARD),
        _ => None,
    }
}

/// Named keys match by exact, case-sensitive name; every editing key but `Insert` exists, no other named key.
pub fn virtual_key(key: &KeyCode) -> Option<VirtualKey> {
    match key {
        KeyCode::Space => Some(vk::SPACE),
        KeyCode::Escape => Some(vk::ESCAPE),
        KeyCode::Return => Some(vk::RETURN),
        KeyCode::Tab => Some(vk::TAB),
        KeyCode::Character(ch) => character_key(ch.to_ascii_lowercase()),
        KeyCode::Function(number) => function_key(*number),
        KeyCode::Named(name) => named_key(name),
    }
}

fn named_key(name: &str) -> Option<VirtualKey> {
    Some(match name {
        "Backspace" => vk::BACKSPACE,
        "Delete" => vk::FORWARD_DELETE,
        "Home" => vk::HOME,
        "End" => vk::END,
        "PageUp" => vk::PAGE_UP,
        "PageDown" => vk::PAGE_DOWN,
        "ArrowLeft" => vk::LEFT_ARROW,
        "ArrowRight" => vk::RIGHT_ARROW,
        "ArrowDown" => vk::DOWN_ARROW,
        "ArrowUp" => vk::UP_ARROW,
        _ => return None,
    })
}

/// The key that yields `v` while Command is held, given what each key yields then: the key
/// that pastes on the active layout. When several do, the US V key wins if it is one of them,
/// then the lowest code; when none does, the US V key.
pub fn paste_key_among(yields: impl IntoIterator<Item = (VirtualKey, String)>) -> VirtualKey {
    yields
        .into_iter()
        .filter(|(_, text)| text == "v")
        .map(|(key, _)| key)
        .min_by_key(|key| (*key != vk::ANSI_V, *key))
        .unwrap_or(vk::ANSI_V)
}

/// Whether macOS turns Fn with `key` into another key before applications see it (Backspace
/// becomes forward Delete, the arrows Home, End and the page keys, Return the keypad Enter),
/// so a shortcut of Fn and it is never seen.
pub(crate) fn replaced_under_fn(key: VirtualKey) -> bool {
    matches!(
        key,
        vk::BACKSPACE
            | vk::RETURN
            | vk::LEFT_ARROW
            | vk::RIGHT_ARROW
            | vk::UP_ARROW
            | vk::DOWN_ARROW
    )
}

/// Whether `key` is one the system flags with Fn whether or not Fn is held (function keys,
/// arrows, Home, End, Page Up/Down, forward Delete). A shortcut on one matches with Fn held too,
/// unless it lists Fn.
pub(crate) fn reached_through_fn(key: VirtualKey) -> bool {
    is_function_key(key)
        || matches!(
            key,
            vk::LEFT_ARROW
                | vk::RIGHT_ARROW
                | vk::UP_ARROW
                | vk::DOWN_ARROW
                | vk::HOME
                | vk::END
                | vk::PAGE_UP
                | vk::PAGE_DOWN
                | vk::FORWARD_DELETE
        )
}
