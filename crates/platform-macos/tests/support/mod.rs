#![allow(dead_code)]

use dictation_types::{
    HotkeyBinding, HotkeyTriggerMode, KeyCode, ModifierKey, ShortcutEvent, ShortcutId,
    ShortcutState,
};
use platform_macos::keys::{flags, vk};
use platform_macos::{Disposition, InputEvent, InputKind, ShortcutMatcher};

pub const SPACE: u16 = vk::SPACE;
/// The key in the place of A on a US keyboard.
pub const KEY_A: u16 = 0x00;
pub const KEY_E: u16 = 0x0E;

pub fn id(name: &str) -> ShortcutId {
    ShortcutId::new(name)
}

pub fn lone(name: &str, modifier: ModifierKey) -> HotkeyBinding {
    HotkeyBinding::standalone_modifier(id(name), modifier, HotkeyTriggerMode::HoldToTalk)
}

pub fn lone_toggle(name: &str, modifier: ModifierKey) -> HotkeyBinding {
    HotkeyBinding::standalone_modifier(id(name), modifier, HotkeyTriggerMode::ToggleOnPress)
}

pub fn combo(name: &str, modifiers: &[ModifierKey], key: KeyCode) -> HotkeyBinding {
    HotkeyBinding::combination(
        id(name),
        modifiers.to_vec(),
        key,
        HotkeyTriggerMode::HoldToTalk,
    )
}

/// Feeds events to a matcher and remembers the time, so each test reads as a sequence of key
/// presses.
pub struct Keys {
    pub matcher: ShortcutMatcher,
    pub now: u64,
}

impl Keys {
    pub fn with(bindings: Vec<HotkeyBinding>) -> Self {
        let mut matcher = ShortcutMatcher::new();
        for binding in bindings {
            matcher.register(binding).expect("binding registers");
        }
        Self {
            matcher,
            now: 1_000,
        }
    }

    pub fn send(&mut self, kind: InputKind, flags: u64) -> (Vec<ShortcutEvent>, Disposition) {
        self.now += 10;
        self.matcher.handle(InputEvent::new(kind, flags, self.now))
    }

    pub fn down(&mut self, key: u16, flags: u64) -> (Vec<ShortcutEvent>, Disposition) {
        self.send(InputKind::KeyDown { key, repeat: false }, flags)
    }

    pub fn repeat(&mut self, key: u16, flags: u64) -> (Vec<ShortcutEvent>, Disposition) {
        self.send(InputKind::KeyDown { key, repeat: true }, flags)
    }

    pub fn up(&mut self, key: u16, flags: u64) -> (Vec<ShortcutEvent>, Disposition) {
        self.send(InputKind::KeyUp { key }, flags)
    }

    /// A modifier key changed; `flags` are the flags after the change, as the system reports them.
    pub fn modifier(&mut self, key: u16, flags: u64) -> (Vec<ShortcutEvent>, Disposition) {
        self.send(InputKind::FlagsChanged { key }, flags)
    }

    pub fn mouse_down(&mut self, button: i64) -> (Vec<ShortcutEvent>, Disposition) {
        self.send(InputKind::MouseDown { button }, 0)
    }

    pub fn mouse_up(&mut self, button: i64) -> (Vec<ShortcutEvent>, Disposition) {
        self.send(InputKind::MouseUp { button }, 0)
    }
}

/// The flags the system sets while Right Option is down.
pub const RIGHT_OPTION: u64 = flags::OPTION | flags::RIGHT_OPTION;
pub const LEFT_OPTION: u64 = flags::OPTION | flags::LEFT_OPTION;
pub const LEFT_CONTROL: u64 = flags::CONTROL | flags::LEFT_CONTROL;
pub const RIGHT_CONTROL: u64 = flags::CONTROL | flags::RIGHT_CONTROL;
pub const LEFT_COMMAND: u64 = flags::COMMAND | flags::LEFT_COMMAND;
pub const LEFT_SHIFT: u64 = flags::SHIFT | flags::LEFT_SHIFT;

/// What a list of events says, as (shortcut, state) pairs.
pub fn states(events: &[ShortcutEvent]) -> Vec<(String, ShortcutState)> {
    events
        .iter()
        .map(|event| (event.binding_id.as_str().to_owned(), event.state))
        .collect()
}

pub fn one(name: &str, state: ShortcutState) -> Vec<(String, ShortcutState)> {
    vec![(name.to_owned(), state)]
}
