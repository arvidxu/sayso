use dictation_types::{AppResult, HotkeyBinding, HotkeyNotice, ShortcutId};

/// Receives shortcut notices one at a time, in order, on the source's own delivery thread, from
/// which it may call back into the source. Once [`HotkeySource::stop_listening`] returns it is
/// not called again; called from inside the listener, the stop notices arrive after it returns.
pub type ShortcutListener = Box<dyn FnMut(HotkeyNotice) + Send>;

/// Global shortcuts: which ones to watch, and a listener told when one goes down or up. What a
/// press means (start, stop, toggle, double-tap) is decided by the listener, not here.
pub trait HotkeySource: Send + Sync {
    /// Replaces a binding with the same ID. Implementations reject bindings that fail
    /// [`HotkeyBinding::validate`], bindings this platform cannot watch, and bindings on the
    /// same keys as another one.
    fn register(&self, binding: HotkeyBinding) -> AppResult<()>;

    fn unregister(&self, id: &ShortcutId) -> AppResult<bool>;

    fn clear(&self) -> AppResult<()>;

    fn list_bindings(&self) -> AppResult<Vec<HotkeyBinding>>;

    /// Starts watching the keyboard and sends every notice to `listener`, replacing any earlier
    /// listener. Fails, without keeping the listener, when the system denies keyboard access.
    fn listen(&self, listener: ShortcutListener) -> AppResult<()>;

    /// A press in progress is interrupted, and the listener hears that before this returns,
    /// unless this is called from inside the listener. Calling it when not watching is not an
    /// error.
    fn stop_listening(&self) -> AppResult<()>;
}
