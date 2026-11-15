#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InsertMethod {
    PasteWithClipboardRestore,
    /// Pastes and leaves the text on the clipboard, so it can be pasted again by hand.
    PasteKeepingText,
    SimulatedTyping,
    CopyOnly,
}

named_values!(InsertMethod, "insert method", {
    PasteWithClipboardRestore => "paste-with-clipboard-restore",
    PasteKeepingText => "paste-keeping-text",
    SimulatedTyping => "simulated-typing",
    CopyOnly => "copy-only",
});

/// How long a paste waits before putting the clipboard back, as the user chose it. A field left
/// `None` uses the configured wait.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PasteRestoreWaits {
    pub quiet_after_read_ms: Option<u64>,
    pub unread_timeout_ms: Option<u64>,
    pub after_failed_paste_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InjectionWarning {
    /// For example, a password field has focus.
    SecureInputActive,
    ElevatedAdminWindow,
}

/// What a paste saw on the clipboard after the paste keys went out. Only whether an
/// application asked for the text can be seen, not whether it put the text in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PasteRead {
    /// An application asked for the text.
    Read,
    /// No application asked for the text.
    Unread,
    /// Something took the text before the paste keys went out. The system keeps a copy from
    /// then on and answers later readers itself, so they cannot be seen.
    TakenBeforeKeys,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InsertOutcome {
    pub method_used: InsertMethod,
    pub clipboard_restored: bool,
    /// `None` when the method does not watch the clipboard for readers, and when the clipboard
    /// held something that could not be read, so there was nothing to put back and no wait.
    pub paste_read: Option<PasteRead>,
    pub fallback_to_clipboard: bool,
    pub warning: Option<InjectionWarning>,
    pub diagnostic: Option<String>,
}

impl InsertOutcome {
    pub fn success(method_used: InsertMethod) -> Self {
        Self {
            method_used,
            clipboard_restored: method_used == InsertMethod::PasteWithClipboardRestore,
            paste_read: None,
            fallback_to_clipboard: false,
            warning: None,
            diagnostic: None,
        }
    }

    pub fn fallback_copied(
        diagnostic: impl Into<String>,
        warning: Option<InjectionWarning>,
    ) -> Self {
        Self {
            method_used: InsertMethod::CopyOnly,
            clipboard_restored: false,
            paste_read: None,
            fallback_to_clipboard: true,
            warning,
            diagnostic: Some(diagnostic.into()),
        }
    }
}
