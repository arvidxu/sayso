use std::error::Error;

use std::fmt;

use crate::PermissionKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    Validation,
    NotFound,
    AlreadyExists,
    PermissionDenied(Option<PermissionKind>),
    AudioDevice,
    AsrEngine,
    LlmEngine,
    TextInjection,
    Hotkey,
    Storage,
    SecretStore,
    ModelStore,
    ChecksumMismatch,
    Network,
    RateLimited,
    Timeout,
    Cancelled,
    Unsupported,
    Internal,
    /// Defined outside this crate, for example by an extension.
    Custom(String),
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation => write!(f, "validation error"),
            Self::NotFound => write!(f, "not found"),
            Self::AlreadyExists => write!(f, "already exists"),
            Self::PermissionDenied(Some(perm)) => write!(f, "permission denied ({perm})"),
            Self::PermissionDenied(None) => write!(f, "permission denied"),
            Self::AudioDevice => write!(f, "audio device error"),
            Self::AsrEngine => write!(f, "ASR engine error"),
            Self::LlmEngine => write!(f, "LLM engine error"),
            Self::TextInjection => write!(f, "text injection error"),
            Self::Hotkey => write!(f, "hotkey error"),
            Self::Storage => write!(f, "storage error"),
            Self::SecretStore => write!(f, "secret store error"),
            Self::ModelStore => write!(f, "model store error"),
            Self::ChecksumMismatch => write!(f, "checksum mismatch"),
            Self::Network => write!(f, "network error"),
            Self::RateLimited => write!(f, "rate limited"),
            Self::Timeout => write!(f, "timeout"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::Unsupported => write!(f, "unsupported operation"),
            Self::Internal => write!(f, "internal error"),
            Self::Custom(name) => write!(f, "{name}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppError {
    pub kind: ErrorKind,
    pub message: String,
    pub retryable: bool,
}

impl AppError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        let retryable = matches!(
            kind,
            ErrorKind::Network | ErrorKind::RateLimited | ErrorKind::Timeout
        );
        Self {
            kind,
            message: message.into(),
            retryable,
        }
    }

    pub fn with_retryable(mut self, retryable: bool) -> Self {
        self.retryable = retryable;
        self
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Validation, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::NotFound, message)
    }

    pub fn already_exists(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::AlreadyExists, message)
    }

    pub fn permission_denied(
        permission: Option<PermissionKind>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(ErrorKind::PermissionDenied(permission), message)
    }

    pub fn audio(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::AudioDevice, message)
    }

    pub fn asr(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::AsrEngine, message)
    }

    pub fn llm(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::LlmEngine, message)
    }

    pub fn injection(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::TextInjection, message)
    }

    pub fn hotkey(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Hotkey, message)
    }

    pub fn storage(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Storage, message)
    }

    pub fn secret(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::SecretStore, message)
    }

    pub fn model(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::ModelStore, message)
    }

    pub fn checksum_mismatch(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::ChecksumMismatch, message)
    }

    pub fn network(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Network, message)
    }

    pub fn rate_limited(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::RateLimited, message)
    }

    pub fn timeout(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Timeout, message)
    }

    pub fn cancelled(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Cancelled, message)
    }

    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Unsupported, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Internal, message)
    }

    pub fn is_cancelled(&self) -> bool {
        self.kind == ErrorKind::Cancelled
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.message.is_empty() {
            write!(f, "{}", self.kind)
        } else {
            write!(f, "{}: {}", self.kind, self.message)
        }
    }
}

impl Error for AppError {}

pub type AppResult<T> = Result<T, AppError>;

/// Redacted in `Debug` and `Display` so keys never reach logs.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct SecretString(String);

impl SecretString {
    pub fn new(secret: impl Into<String>) -> Self {
        Self(secret.into())
    }

    pub fn try_new(secret: impl Into<String>) -> AppResult<Self> {
        let value = secret.into();
        if value.trim().is_empty() {
            return Err(AppError::validation("secret value must not be empty"));
        }
        Ok(Self(value))
    }

    pub fn expose_secret(&self) -> &str {
        &self.0
    }

    /// Whitespace-only counts as empty, as in [`Self::try_new`].
    pub fn is_empty(&self) -> bool {
        self.0.trim().is_empty()
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretString([REDACTED])")
    }
}

impl fmt::Display for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl From<String> for SecretString {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for SecretString {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}
