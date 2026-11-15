use std::sync::Mutex;

use dictation_api::ContextProvider;
use dictation_types::{ActiveAppInfo, AppContext, AppError, AppResult, AppScene, ContextQuery};

/// Answers every query with everything, so a shortcut that returned the wrong field or asked
/// for too much would be caught by the recorded queries.
#[derive(Default)]
struct RecordingContext {
    queries: Mutex<Vec<ContextQuery>>,
}

impl ContextProvider for RecordingContext {
    fn snapshot(&self, query: &ContextQuery) -> AppResult<AppContext> {
        self.queries.lock().unwrap().push(*query);
        Ok(AppContext::new(AppScene::Document)
            .with_active_app(ActiveAppInfo::new("Editor"))
            .with_selected_text("the selection")
            .with_clipboard_text("the clipboard"))
    }
}

#[test]
fn the_active_app_shortcut_asks_only_for_the_active_app() {
    let context = RecordingContext::default();
    let app = context.active_app().unwrap();
    assert_eq!(app.map(|a| a.app_name).as_deref(), Some("Editor"));
    assert_eq!(
        *context.queries.lock().unwrap(),
        [ContextQuery {
            include_active_app: true,
            include_selected_text: false,
            include_clipboard: false,
        }]
    );
}

#[test]
fn the_selection_shortcut_asks_only_for_the_selection() {
    let context = RecordingContext::default();
    assert_eq!(
        context.selected_text().unwrap().as_deref(),
        Some("the selection")
    );
    assert_eq!(
        *context.queries.lock().unwrap(),
        [ContextQuery {
            include_active_app: false,
            include_selected_text: true,
            include_clipboard: false,
        }]
    );
}

#[test]
fn the_clipboard_shortcut_asks_only_for_the_clipboard() {
    let context = RecordingContext::default();
    assert_eq!(
        context.clipboard_text().unwrap().as_deref(),
        Some("the clipboard")
    );
    assert_eq!(
        *context.queries.lock().unwrap(),
        [ContextQuery {
            include_active_app: false,
            include_selected_text: false,
            include_clipboard: true,
        }]
    );
}

struct BrokenContext;

impl ContextProvider for BrokenContext {
    fn snapshot(&self, _query: &ContextQuery) -> AppResult<AppContext> {
        Err(AppError::permission_denied(None, "accessibility is off"))
    }
}

#[test]
fn a_context_shortcut_fails_when_the_context_cannot_be_read() {
    assert!(BrokenContext.active_app().is_err());
    assert!(BrokenContext.selected_text().is_err());
    assert!(BrokenContext.clipboard_text().is_err());
}
