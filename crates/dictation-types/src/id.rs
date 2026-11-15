use std::fmt;

use crate::{AppError, AppResult};

macro_rules! define_id {
    ($(#[$meta:meta])* $name:ident, $label:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            /// Trims `value`; may be empty, so use [`Self::try_new`] for untrusted input.
            pub fn new(value: impl Into<String>) -> Self {
                let raw = value.into();
                let trimmed = raw.trim();
                if trimmed.len() == raw.len() {
                    Self(raw)
                } else {
                    Self(trimmed.to_owned())
                }
            }

            /// Trims `value` and rejects it when nothing is left.
            pub fn try_new(value: impl Into<String>) -> AppResult<Self> {
                let id = Self::new(value);
                if id.is_empty() {
                    return Err(AppError::validation(concat!($label, " must not be empty")));
                }
                Ok(id)
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn into_inner(self) -> String {
                self.0
            }

            pub fn is_empty(&self) -> bool {
                self.0.is_empty()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self::new(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self::new(value)
            }
        }
    };
}

define_id!(TranscriptId, "transcript ID");

define_id!(HistoryId, "history ID");

define_id!(ModeId, "mode ID");

define_id!(TermId, "term ID");

define_id!(RuleId, "rule ID");

define_id!(ModelId, "model ID");

define_id!(ProviderId, "provider ID");

define_id!(DeviceId, "device ID");

define_id!(FeatureId, "feature ID");

define_id!(ExtensionId, "extension ID");

define_id!(ShortcutId, "shortcut ID");
