use std::sync::Mutex;

use dictation_api::TextInjector;
use dictation_types::{
    AppError, AppResult, ErrorKind, InjectionWarning, InsertMethod, InsertOutcome,
};

#[derive(Default)]
struct RecordingInjector {
    calls: Mutex<Vec<(String, InsertMethod)>>,
    raised_while_copying: Option<InjectionWarning>,
    fail: bool,
}

impl TextInjector for RecordingInjector {
    fn insert(&self, text: &str, method: InsertMethod) -> AppResult<InsertOutcome> {
        self.calls.lock().unwrap().push((text.to_owned(), method));
        if self.fail {
            return Err(AppError::injection("clipboard unavailable").with_retryable(true));
        }
        let mut outcome = InsertOutcome::success(method);
        outcome.warning = self.raised_while_copying.clone();
        Ok(outcome)
    }
}

#[test]
fn copy_fallback_only_copies() {
    let injector = RecordingInjector::default();
    injector
        .copy_fallback("Olá, мир, 世界", "paste was blocked", None)
        .unwrap();
    assert_eq!(
        *injector.calls.lock().unwrap(),
        [("Olá, мир, 世界".to_owned(), InsertMethod::CopyOnly)]
    );
}

#[test]
fn copy_fallback_records_the_reason_and_marks_the_fallback() {
    let injector = RecordingInjector::default();
    let outcome = injector
        .copy_fallback("नमस्ते", "paste was blocked", None)
        .unwrap();
    assert!(outcome.fallback_to_clipboard);
    assert_eq!(outcome.diagnostic.as_deref(), Some("paste was blocked"));
    assert_eq!(outcome.method_used, InsertMethod::CopyOnly);
    assert!(!outcome.clipboard_restored);
    assert_eq!(outcome.warning, None);
}

#[test]
fn copy_fallback_reports_the_warning_it_was_given() {
    let injector = RecordingInjector::default();
    let outcome = injector
        .copy_fallback("x", "why", Some(InjectionWarning::ElevatedAdminWindow))
        .unwrap();
    assert_eq!(outcome.warning, Some(InjectionWarning::ElevatedAdminWindow));
}

#[test]
fn copy_fallback_does_not_overwrite_a_warning_raised_while_copying() {
    let injector = RecordingInjector {
        raised_while_copying: Some(InjectionWarning::SecureInputActive),
        ..RecordingInjector::default()
    };
    let outcome = injector
        .copy_fallback("x", "why", Some(InjectionWarning::ElevatedAdminWindow))
        .unwrap();
    assert_eq!(outcome.warning, Some(InjectionWarning::SecureInputActive));

    let without_passed_warning = injector.copy_fallback("x", "why", None).unwrap();
    assert_eq!(
        without_passed_warning.warning,
        Some(InjectionWarning::SecureInputActive)
    );
    assert!(without_passed_warning.fallback_to_clipboard);
}

#[test]
fn copy_fallback_fails_when_copying_fails() {
    let injector = RecordingInjector {
        fail: true,
        ..RecordingInjector::default()
    };
    let err = injector.copy_fallback("x", "why", None).unwrap_err();
    assert_eq!(err.kind, ErrorKind::TextInjection);
    assert!(err.retryable, "whether to try again is kept");
    assert_eq!(injector.calls.lock().unwrap().len(), 1);
}

#[test]
fn an_injector_reports_no_warning_by_default() {
    assert_eq!(RecordingInjector::default().check_injection_warning(), None);
}
