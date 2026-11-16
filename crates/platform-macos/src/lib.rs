//! Global shortcuts, putting text into the application in front, and the system permissions both
//! need. The matching rules and insertion steps are plain code that builds on every system.

mod clipboard_contents;
mod clock;
mod config;
mod delivery;
mod hotkeys;
mod input;
pub mod insertion;
mod insertion_parts;
mod insertion_wait;
pub mod keys;
mod keys_data;
mod keys_down;
mod shortcut_matcher;
mod trigger;

#[cfg(target_os = "macos")]
pub mod clipboard;
#[cfg(target_os = "macos")]
mod event_tap;
#[cfg(target_os = "macos")]
mod key_state;
#[cfg(target_os = "macos")]
pub mod keyboard;
#[cfg(target_os = "macos")]
mod keyboard_layout;
#[cfg(target_os = "macos")]
pub mod main_thread;
#[cfg(target_os = "macos")]
mod permissions;
#[cfg(target_os = "macos")]
pub mod secure_input;

pub use config::{BUNDLED_CONFIG, InsertionTuning, PlatformConfig};
pub use hotkeys::MacosHotkeySource;
pub use input::{Disposition, InputEvent, InputKind};
pub use insertion::{
    Clipboard, ClipboardContents, ClipboardSnapshot, Inserter, Keyboard, MainThread, OfferNews,
    SecureInput, TextOffer,
};
pub use shortcut_matcher::{KeyState, ShortcutMatcher};

#[cfg(target_os = "macos")]
pub use permissions::MacosPermissions;

#[cfg(target_os = "macos")]
pub type MacosTextInjector = Inserter<
    clipboard::SystemClipboard,
    keyboard::SystemKeyboard,
    secure_input::SystemSecureInput,
    main_thread::SystemMainThread,
>;

#[cfg(target_os = "macos")]
impl MacosTextInjector {
    pub fn new(tuning: InsertionTuning) -> Self {
        Self::with_parts(
            clipboard::SystemClipboard::general(),
            keyboard::SystemKeyboard,
            secure_input::SystemSecureInput,
            main_thread::SystemMainThread,
            tuning,
        )
    }
}
