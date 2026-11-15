use crate::{AppError, AppResult, ModeId, ShortcutId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HotkeyTriggerMode {
    HoldToTalk,
    ToggleOnPress,
    /// Lone modifier only; its single press stays with the system.
    DoubleTapToggle,
}

/// `Alt` is labelled Option on some keyboards; `Meta` is Command, the system logo key, or
/// Super.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ModifierKey {
    Fn,
    CapsLock,
    LeftShift,
    RightShift,
    LeftControl,
    RightControl,
    LeftAlt,
    RightAlt,
    LeftMeta,
    RightMeta,
    MouseSideForward,
    MouseSideBack,
}

/// Read the same on every platform, so a shortcut stays on the same key across them.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum KeyCode {
    Space,
    Escape,
    Return,
    Tab,
    /// The physical key at this character's position on a US layout, whatever layout is
    /// active; case is ignored.
    Character(char),
    Function(u8),
    /// A name exactly as `is_known_key_name` accepts it, case included. A platform refuses a
    /// name it has no key for.
    Named(String),
}

/// With Shift they select, paste or delete, so they need Control, Alt, Meta or Fn.
const EDITING_KEYS: &[&str] = &[
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
];

/// Named keys that neither type nor edit.
const OTHER_NAMED_KEYS: &[&str] = &["PrintScreen", "ScrollLock", "Pause", "ContextMenu", "Help"];

/// Case-sensitive.
pub fn is_known_key_name(name: &str) -> bool {
    EDITING_KEYS.contains(&name) || OTHER_NAMED_KEYS.contains(&name)
}

impl KeyCode {
    /// Keys where Shift or Caps Lock alone would catch ordinary typing.
    pub fn is_typing_key(&self) -> bool {
        match self {
            Self::Character(_) | Self::Space | Self::Tab | Self::Return => true,
            Self::Named(name) => EDITING_KEYS.contains(&name.as_str()),
            Self::Escape | Self::Function(_) => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotkeyBinding {
    pub id: ShortcutId,
    pub modifiers: Vec<ModifierKey>,
    pub key: Option<KeyCode>,
    pub trigger_mode: HotkeyTriggerMode,
    /// `None` uses the currently active mode.
    pub mode_id: Option<ModeId>,
}

impl HotkeyBinding {
    pub fn standalone_modifier(
        id: ShortcutId,
        modifier: ModifierKey,
        trigger_mode: HotkeyTriggerMode,
    ) -> Self {
        Self {
            id,
            modifiers: vec![modifier],
            key: None,
            trigger_mode,
            mode_id: None,
        }
    }

    pub fn combination(
        id: ShortcutId,
        modifiers: Vec<ModifierKey>,
        key: KeyCode,
        trigger_mode: HotkeyTriggerMode,
    ) -> Self {
        Self {
            id,
            modifiers,
            key: Some(key),
            trigger_mode,
            mode_id: None,
        }
    }

    pub fn with_mode(mut self, mode_id: ModeId) -> Self {
        self.mode_id = Some(mode_id);
        self
    }

    pub fn is_standalone_modifier(&self) -> bool {
        self.key.is_none() && self.modifiers.len() == 1
    }

    /// A lone modifier, a function key, or a key with a modifier (Control, Alt, Meta or Fn for
    /// typing keys); double-tap only on a lone modifier.
    pub fn validate(&self) -> AppResult<()> {
        if self.id.is_empty() {
            return Err(AppError::validation("shortcut ID must not be empty"));
        }
        if self.key.is_none() && self.modifiers.len() != 1 {
            return Err(AppError::validation(
                "a shortcut needs a key, or exactly one modifier on its own",
            ));
        }
        match &self.key {
            Some(KeyCode::Function(number)) if !(1..=24).contains(number) => {
                return Err(AppError::validation(
                    "function keys are numbered from 1 to 24",
                ));
            }
            Some(KeyCode::Named(name)) if !is_known_key_name(name) => {
                return Err(AppError::validation(format!(
                    "{name:?} is not a key name this shortcut can use; names are case-sensitive, \
                     and space, tab, return and escape have keys of their own"
                )));
            }
            Some(KeyCode::Character(ch)) if ch.is_whitespace() || ch.is_control() => {
                return Err(AppError::validation(
                    "use the dedicated key for space, tab, return and escape",
                ));
            }
            Some(KeyCode::Function(_)) | None => {}
            Some(_) if self.modifiers.is_empty() => {
                return Err(AppError::validation(
                    "a shortcut on an ordinary key needs at least one modifier",
                ));
            }
            // Shift and Caps Lock alone would catch ordinary typing and selecting.
            Some(key)
                if key.is_typing_key()
                    && self.modifiers.iter().all(|modifier| {
                        matches!(
                            modifier,
                            ModifierKey::LeftShift
                                | ModifierKey::RightShift
                                | ModifierKey::CapsLock
                                | ModifierKey::MouseSideForward
                                | ModifierKey::MouseSideBack
                        )
                    }) =>
            {
                return Err(AppError::validation(
                    "a shortcut on a typing or editing key needs Control, Alt, Meta or Fn",
                ));
            }
            Some(_) => {}
        }
        if self.trigger_mode == HotkeyTriggerMode::DoubleTapToggle && self.key.is_some() {
            return Err(AppError::validation(
                "double-tap applies to a modifier on its own",
            ));
        }
        let mut seen: Vec<ModifierKey> = Vec::new();
        for modifier in &self.modifiers {
            if seen.contains(modifier) {
                return Err(AppError::validation(
                    "a shortcut lists the same modifier twice",
                ));
            }
            seen.push(*modifier);
        }
        Ok(())
    }
}

/// Raw events only; the dictation flow decides what a press means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShortcutState {
    Pressed,
    Released,
    /// Ended without a release: another key joined it, or tracking was lost. No release
    /// follows.
    Interrupted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutEvent {
    pub binding_id: ShortcutId,
    pub state: ShortcutState,
    /// Monotonic clock; only differences between events are meaningful.
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HotkeyNotice {
    Shortcut(ShortcutEvent),
    /// The system stopped the watching, for example when the permission to watch the keyboard
    /// was revoked; the listener did not ask for it. No shortcut works until listening restarts.
    /// A press in progress is interrupted first.
    WatchingStopped,
}
