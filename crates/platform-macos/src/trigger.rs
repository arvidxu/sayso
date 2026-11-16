use std::collections::BTreeSet;

use dictation_types::{
    AppError, AppResult, HotkeyBinding, HotkeyTriggerMode, ModifierKey, ShortcutId,
};

use crate::keys::{VirtualKey, mouse_button, reached_through_fn, replaced_under_fn, virtual_key};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Trigger {
    /// A key pressed while exactly these modifiers are held.
    Combination {
        key: VirtualKey,
        modifiers: BTreeSet<ModifierKey>,
    },
    /// A modifier pressed on its own.
    Modifier(ModifierKey),
    /// It switches a lock and is never held, so a press is reported as going
    /// down and straight back up.
    CapsLock,
    /// A mouse side button pressed on its own.
    MouseButton(i64),
}

impl Trigger {
    pub(crate) fn of(binding: &HotkeyBinding) -> AppResult<Self> {
        binding.validate()?;
        let Some(key) = &binding.key else {
            return Self::alone(binding.modifiers[0], binding.trigger_mode);
        };
        let key = virtual_key(key).ok_or_else(|| {
            AppError::validation(format!("macOS has no key {key:?} to use in a shortcut"))
        })?;
        if binding
            .modifiers
            .iter()
            .any(|modifier| *modifier == ModifierKey::CapsLock || mouse_button(*modifier).is_some())
        {
            return Err(AppError::validation(
                "on macOS, Caps Lock and the mouse side buttons work only on their own",
            ));
        }
        if replaced_under_fn(key) && binding.modifiers.contains(&ModifierKey::Fn) {
            return Err(AppError::validation(
                "macOS turns Fn with Backspace, an arrow or Return into \
                 another key, so this shortcut could never be pressed",
            ));
        }
        Ok(Self::Combination {
            key,
            modifiers: binding.modifiers.iter().copied().collect(),
        })
    }

    fn alone(modifier: ModifierKey, mode: HotkeyTriggerMode) -> AppResult<Self> {
        if modifier == ModifierKey::CapsLock {
            return if mode == HotkeyTriggerMode::HoldToTalk {
                Err(AppError::validation(
                    "Caps Lock cannot be held on macOS: it only switches on and off",
                ))
            } else {
                Ok(Self::CapsLock)
            };
        }
        let Some(button) = mouse_button(modifier) else {
            return Ok(Self::Modifier(modifier));
        };
        if mode == HotkeyTriggerMode::DoubleTapToggle {
            // A shortcut button's presses are kept from the application, so double-tap could
            // not leave a single press to it as the mode promises.
            return Err(AppError::validation(
                "double-tap on a mouse side button would keep every single press from the application",
            ));
        }
        Ok(Self::MouseButton(button))
    }

    /// Whether the two triggers go off on the same keys, so only one of them could ever be
    /// seen. A key that laptop keyboards reach through Fn matches with Fn held whether or not
    /// the shortcut lists it, so for those keys Fn does not tell two shortcuts apart.
    pub(crate) fn same_keys(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Combination { key, modifiers },
                Self::Combination {
                    key: other_key,
                    modifiers: other_modifiers,
                },
            ) if key == other_key && reached_through_fn(*key) => {
                let without_fn = |set: &BTreeSet<ModifierKey>| {
                    let mut set = set.clone();
                    set.remove(&ModifierKey::Fn);
                    set
                };
                without_fn(modifiers) == without_fn(other_modifiers)
            }
            _ => self == other,
        }
    }
}

/// The shortcut that is down right now.
#[derive(Debug)]
pub(crate) enum Active {
    Combination {
        id: ShortcutId,
        key: VirtualKey,
        modifiers: BTreeSet<ModifierKey>,
    },
    Modifier {
        id: ShortcutId,
        modifier: ModifierKey,
    },
    MouseButton {
        id: ShortcutId,
        button: i64,
    },
}

impl Active {
    pub(crate) fn id(&self) -> &ShortcutId {
        match self {
            Self::Combination { id, .. }
            | Self::Modifier { id, .. }
            | Self::MouseButton { id, .. } => id,
        }
    }
}
