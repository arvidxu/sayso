use dictation_types::{AppResult, SecretString};

/// Storage for credentials, kept out of ordinary settings and logs.
pub trait SecretStore: Send + Sync {
    fn get_secret(&self, key: &str) -> AppResult<Option<SecretString>>;

    fn set_secret(&self, key: &str, secret: SecretString) -> AppResult<()>;

    fn delete_secret(&self, key: &str) -> AppResult<bool>;

    /// Returns `true` if a non-empty secret is stored under `key`.
    fn has_secret(&self, key: &str) -> AppResult<bool> {
        Ok(self.get_secret(key)?.is_some_and(|s| !s.is_empty()))
    }
}
