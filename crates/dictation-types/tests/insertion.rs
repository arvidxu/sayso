use dictation_types::{InjectionWarning, InsertMethod, InsertOutcome};

#[test]
fn a_successful_paste_reports_the_clipboard_as_restored() {
    let paste = InsertOutcome::success(InsertMethod::PasteWithClipboardRestore);
    assert!(paste.clipboard_restored);
    assert_eq!(paste.paste_read, None);
    assert!(!paste.fallback_to_clipboard);
    assert_eq!(paste.warning, None);
    assert_eq!(paste.diagnostic, None);

    for method in [
        InsertMethod::PasteKeepingText,
        InsertMethod::SimulatedTyping,
        InsertMethod::CopyOnly,
    ] {
        let outcome = InsertOutcome::success(method);
        assert_eq!(outcome.method_used, method);
        assert!(!outcome.clipboard_restored);
        assert!(!outcome.fallback_to_clipboard);
    }
}

#[test]
fn a_copy_fallback_outcome_says_so_and_why() {
    let outcome =
        InsertOutcome::fallback_copied("paste blocked", Some(InjectionWarning::SecureInputActive));
    assert_eq!(outcome.method_used, InsertMethod::CopyOnly);
    assert!(outcome.fallback_to_clipboard);
    assert!(!outcome.clipboard_restored);
    assert_eq!(outcome.paste_read, None);
    assert_eq!(outcome.diagnostic.as_deref(), Some("paste blocked"));
    assert_eq!(outcome.warning, Some(InjectionWarning::SecureInputActive));
}
