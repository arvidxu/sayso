use dictation_types::{AppError, AppResult, InjectionWarning, InsertMethod, InsertOutcome};

pub trait TextInjector: Send + Sync {
    fn insert(&self, text: &str, method: InsertMethod) -> AppResult<InsertOutcome>;

    /// Anything in the environment that would block or degrade insertion right now.
    fn check_injection_warning(&self) -> Option<InjectionWarning> {
        None
    }

    /// Copies `text` to the clipboard after direct insertion failed, recording why. A warning
    /// raised while copying is kept in preference to `warning`; why insertion failed is kept
    /// in front of anything copying reports, and in the error when copying fails too.
    fn copy_fallback(
        &self,
        text: &str,
        diagnostic: &str,
        warning: Option<InjectionWarning>,
    ) -> AppResult<InsertOutcome> {
        let mut outcome = self
            .insert(text, InsertMethod::CopyOnly)
            .map_err(|err| AppError {
                message: format!(
                    "{diagnostic}; copying to the clipboard failed too: {}",
                    err.message
                ),
                ..err
            })?;
        outcome.fallback_to_clipboard = true;
        outcome.diagnostic = Some(match outcome.diagnostic.take() {
            Some(copying) => format!("{diagnostic}; {copying}"),
            None => diagnostic.to_owned(),
        });
        if outcome.warning.is_none() {
            outcome.warning = warning;
        }
        Ok(outcome)
    }
}
