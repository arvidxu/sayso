use dictation_types::{AppError, AppResult};
use objc2_core_foundation::CFRetained;
use objc2_core_graphics::{
    CGEvent, CGEventFlags, CGEventSource, CGEventSourceStateID, CGEventTapLocation,
    CGPreflightPostEventAccess,
};

use crate::insertion::Keyboard;
use crate::keyboard_layout::paste_key;

/// Marks the events this package sends, so the shortcut watcher can tell them from the user's
/// own keys. Any number works as long as no other program uses it; this one spells "SAYS".
pub(crate) const OWN_EVENT_MARK: i64 = 0x5341_5953;

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemKeyboard;

impl SystemKeyboard {
    /// A source of its own, so the events do not pick up the modifiers the user is holding,
    /// marked as ours.
    fn source() -> AppResult<CFRetained<CGEventSource>> {
        let source = CGEventSource::new(CGEventSourceStateID::Private).ok_or_else(|| {
            AppError::injection("the system could not make a keyboard event source")
        })?;
        CGEventSource::set_user_data(Some(&source), OWN_EVENT_MARK);
        Ok(source)
    }

    fn key_event(
        source: &CGEventSource,
        key: u16,
        down: bool,
        flags: CGEventFlags,
    ) -> AppResult<CFRetained<CGEvent>> {
        let event = CGEvent::new_keyboard_event(Some(source), key, down)
            .ok_or_else(|| AppError::injection("the system could not make a keyboard event"))?;
        CGEvent::set_flags(Some(&event), flags);
        Ok(event)
    }

    fn post(event: &CGEvent) {
        CGEvent::post(CGEventTapLocation::HIDEventTap, Some(event));
    }
}

impl Keyboard for SystemKeyboard {
    fn can_send(&self) -> bool {
        CGPreflightPostEventAccess()
    }

    /// Sends Command and the key that yields V with it on the active layout.
    fn paste(&self) -> AppResult<()> {
        let key = paste_key();
        let source = Self::source()?;
        let command = CGEventFlags::MaskCommand;
        let down = Self::key_event(&source, key, true, command)?;
        let up = Self::key_event(&source, key, false, command)?;
        Self::post(&down);
        Self::post(&up);
        Ok(())
    }

    fn type_units(&self, text: &[u16]) -> AppResult<()> {
        let source = Self::source()?;
        // Both events are made before either is sent, so a failure never leaves a key down.
        let mut events = Vec::with_capacity(2);
        for down in [true, false] {
            let event = Self::key_event(&source, 0, down, CGEventFlags::empty())?;
            // SAFETY: the pointer and length describe `text`, which outlives the call.
            unsafe {
                CGEvent::keyboard_set_unicode_string(Some(&event), text.len() as _, text.as_ptr());
            }
            events.push(event);
        }
        for event in &events {
            Self::post(event);
        }
        Ok(())
    }
}
