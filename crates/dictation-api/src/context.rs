use dictation_types::{ActiveAppInfo, AppContext, AppResult, ContextQuery};

/// The foreground application, the selection, and the clipboard.
pub trait ContextProvider: Send + Sync {
    /// Reads the context, touching only the fields `query` asks for.
    fn snapshot(&self, query: &ContextQuery) -> AppResult<AppContext>;

    fn active_app(&self) -> AppResult<Option<ActiveAppInfo>> {
        Ok(self.snapshot(&ContextQuery::minimal())?.active_app)
    }

    fn selected_text(&self) -> AppResult<Option<String>> {
        let query = ContextQuery {
            include_active_app: false,
            include_selected_text: true,
            include_clipboard: false,
        };
        Ok(self.snapshot(&query)?.selected_text)
    }

    fn clipboard_text(&self) -> AppResult<Option<String>> {
        let query = ContextQuery {
            include_active_app: false,
            include_selected_text: false,
            include_clipboard: true,
        };
        Ok(self.snapshot(&query)?.clipboard_text)
    }
}
