use dictation_types::{AppResult, UserSettings};

/// Saved settings. The key-value methods are a separate space for options owned by one part of
/// the app; prefix keys with that part's name.
pub trait SettingsStore: Send + Sync {
    /// The saved settings, or `None` when nothing has been saved yet. There are no built-in
    /// settings to fall back to: the app decides how to start, for example from the system
    /// language.
    fn load(&self) -> AppResult<Option<UserSettings>>;

    fn save(&self, settings: &UserSettings) -> AppResult<()>;

    fn get_value(&self, key: &str) -> AppResult<Option<String>>;

    fn set_value(&self, key: &str, value: &str) -> AppResult<()>;

    fn remove_value(&self, key: &str) -> AppResult<bool>;
}
