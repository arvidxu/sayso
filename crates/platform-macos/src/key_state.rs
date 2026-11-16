use objc2_core_graphics::{CGEventSource, CGEventSourceStateID, CGMouseButton};

use crate::keys::VirtualKey;
use crate::shortcut_matcher::KeyState;

/// The state of the keyboard and mouse as the hardware reports it.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemKeyState;

impl KeyState for SystemKeyState {
    fn key_down(&self, key: VirtualKey) -> bool {
        CGEventSource::key_state(CGEventSourceStateID::HIDSystemState, key)
    }

    fn button_down(&self, button: i64) -> bool {
        u32::try_from(button).is_ok_and(|button| {
            CGEventSource::button_state(CGEventSourceStateID::HIDSystemState, CGMouseButton(button))
        })
    }
}
