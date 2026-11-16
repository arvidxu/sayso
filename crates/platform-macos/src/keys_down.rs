use std::collections::BTreeSet;

use dictation_types::ModifierKey;

use crate::input::{InputEvent, InputKind};
use crate::keys::{
    SIDED_MODIFIERS, VirtualKey, flags, modifier_of_key, reached_through_fn, sided_flags,
};

/// The main and the secondary mouse button, which click and drag.
const MAIN_BUTTONS: [i64; 2] = [0, 1];

/// Asks the system whether a key or mouse button is down right now, to correct what the
/// matcher believes after a release it never saw.
pub trait KeyState: Send {
    fn key_down(&self, key: VirtualKey) -> bool;
    fn button_down(&self, button: i64) -> bool;
}

/// What is down, as far as the events seen tell.
#[derive(Default)]
pub(crate) struct KeysDown {
    /// Fn included, Caps Lock not.
    pub(crate) held: BTreeSet<ModifierKey>,
    /// Other keys held down whose events went on to the application.
    pub(crate) others: BTreeSet<VirtualKey>,
    /// Mouse buttons held down whose events went on to the application, the main two
    /// included, as during a click or a drag.
    pub(crate) buttons: BTreeSet<i64>,
    /// Keys and buttons whose press was kept from the application, so their release and
    /// repeats are kept from it too.
    pub(crate) swallowed_keys: BTreeSet<VirtualKey>,
    pub(crate) swallowed_buttons: BTreeSet<i64>,
    pub(crate) caps_lock_on: Option<bool>,
    key_state: Option<Box<dyn KeyState>>,
}

impl KeysDown {
    pub(crate) fn with_key_state(key_state: Box<dyn KeyState>) -> Self {
        Self {
            key_state: Some(key_state),
            ..Self::default()
        }
    }

    /// Keys whose press was kept from the application stay kept until they come up.
    pub(crate) fn forget(&mut self) {
        self.held.clear();
        self.others.clear();
        self.buttons.clear();
        self.caps_lock_on = None;
    }

    pub(crate) fn sync_held(&mut self, input: InputEvent) {
        for &(modifier, shared, own, other) in SIDED_MODIFIERS {
            if input.flags & shared == 0 {
                self.held.remove(&modifier);
            } else if input.flags & (own | other) != 0 {
                self.set_held(modifier, input.flags & own != 0);
            }
        }
        let changed = match input.kind {
            InputKind::FlagsChanged { key } => modifier_of_key(key),
            _ => None,
        };
        match changed {
            Some(ModifierKey::Fn) => self.set_held(ModifierKey::Fn, input.flags & flags::FN != 0),
            Some(modifier) => {
                // An event without side flags, as a program may make: the key says the side.
                if let Some((shared, own, other)) = sided_flags(modifier)
                    && input.flags & (own | other) == 0
                {
                    self.set_held(modifier, input.flags & shared != 0);
                }
            }
            None => {}
        }
        if changed != Some(ModifierKey::Fn) && input.flags & flags::FN == 0 {
            self.held.remove(&ModifierKey::Fn);
        }
    }

    /// Fn held for a key that laptop
    /// keyboards reach through Fn does not count against a shortcut that does not list it.
    pub(crate) fn holding_exactly(
        &self,
        key: VirtualKey,
        modifiers: &BTreeSet<ModifierKey>,
    ) -> bool {
        if reached_through_fn(key) && !modifiers.contains(&ModifierKey::Fn) {
            let mut held = self.held.clone();
            held.remove(&ModifierKey::Fn);
            held == *modifiers
        } else {
            self.held == *modifiers
        }
    }

    /// Whether no other key or mouse button is down (the shortcut's own key and the main buttons
    /// included), after dropping any the system says came up unseen. A main button the system says
    /// is down counts too.
    pub(crate) fn nothing_else_down(&mut self) -> bool {
        let mut main_button_down = false;
        if let Some(state) = &self.key_state {
            self.others.retain(|key| state.key_down(*key));
            self.buttons.retain(|button| state.button_down(*button));
            self.swallowed_keys.retain(|key| state.key_down(*key));
            self.swallowed_buttons
                .retain(|button| state.button_down(*button));
            main_button_down = MAIN_BUTTONS.iter().any(|button| state.button_down(*button));
        }
        !main_button_down
            && self.others.is_empty()
            && self.buttons.is_empty()
            && self.swallowed_keys.is_empty()
            && self.swallowed_buttons.is_empty()
    }

    fn set_held(&mut self, modifier: ModifierKey, down: bool) {
        if down {
            self.held.insert(modifier);
        } else {
            self.held.remove(&modifier);
        }
    }
}
