use configuration::PackageConfig;
use dictation_types::{AppError, AppResult, UserSettings};
use serde::Deserialize;
use serde_json::{Value, from_str, from_value};

pub const BUNDLED_CONFIG: &str = include_str!("../platform-macos.conf.json");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformConfig {
    pub insertion: InsertionTuning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InsertionTuning {
    /// How long after the last application asked for the pasted text the clipboard is put back.
    /// An application may ask more than once for one paste, in more than one type.
    pub restore_quiet_after_read_ms: u64,
    /// How long after the paste keys went out the clipboard is put back when no application has
    /// asked for the text. A busy application may take a while to handle the keys.
    pub restore_unread_timeout_ms: u64,
    /// How long the clipboard is left with the text when sending the paste keys failed.
    pub restore_after_failed_paste_ms: u64,
}

impl InsertionTuning {
    /// A zero wait puts the clipboard back before the application has handled the paste keys,
    /// so it pastes what was there before instead of the text.
    pub fn validate(&self) -> AppResult<()> {
        let waits = [
            ("restoreQuietAfterReadMs", self.restore_quiet_after_read_ms),
            ("restoreUnreadTimeoutMs", self.restore_unread_timeout_ms),
            (
                "restoreAfterFailedPasteMs",
                self.restore_after_failed_paste_ms,
            ),
        ];
        for (key, ms) in waits {
            if ms == 0 {
                return Err(AppError::validation(format!(
                    "{key} must be greater than zero"
                )));
            }
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConfigFile {
    insertion: InsertionFile,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InsertionFile {
    restore_quiet_after_read_ms: u64,
    restore_unread_timeout_ms: u64,
    restore_after_failed_paste_ms: u64,
}

const GROUPS: &[&str] = &["insertion"];

/// Refuses a group written as a list of values in order: every value has to be named.
fn named_values(value: &Value, what: &str) -> AppResult<()> {
    if value.is_object() {
        Ok(())
    } else {
        Err(AppError::validation(format!(
            "{what} must be an object with a name for every value"
        )))
    }
}

impl PackageConfig for PlatformConfig {
    const NAME: &'static str = "platform-macos";
    const BUNDLED: &'static str = BUNDLED_CONFIG;

    fn from_json(text: &str) -> AppResult<Self> {
        Self::from_json(text)
    }
}

impl PlatformConfig {
    pub fn bundled() -> AppResult<Self> {
        Self::from_json(BUNDLED_CONFIG)
    }

    /// A missing or unknown key, or a
    /// value of the wrong kind, is an error naming what is wrong.
    pub fn from_json(text: &str) -> AppResult<Self> {
        let invalid =
            |err| AppError::validation(format!("the macOS configuration is not valid: {err}"));
        let value: Value = from_str(text).map_err(invalid)?;
        named_values(&value, "the macOS configuration")?;
        for group in GROUPS {
            if let Some(group_value) = value.get(group) {
                named_values(group_value, group)?;
            }
        }
        let file: ConfigFile = from_value(value).map_err(invalid)?;
        let insertion = InsertionTuning {
            restore_quiet_after_read_ms: file.insertion.restore_quiet_after_read_ms,
            restore_unread_timeout_ms: file.insertion.restore_unread_timeout_ms,
            restore_after_failed_paste_ms: file.insertion.restore_after_failed_paste_ms,
        };
        insertion.validate().map_err(|err| {
            AppError::validation(format!(
                "the macOS configuration is not valid: insertion.{}",
                err.message
            ))
        })?;
        Ok(Self { insertion })
    }

    /// The insertion tuning for a user with `settings`: each wait as the user chose it, or as
    /// configured when the user has not chosen.
    pub fn insertion_for(&self, settings: &UserSettings) -> AppResult<InsertionTuning> {
        let chosen = settings.paste_restore_waits;
        let configured = &self.insertion;
        let tuning = InsertionTuning {
            restore_quiet_after_read_ms: chosen
                .quiet_after_read_ms
                .unwrap_or(configured.restore_quiet_after_read_ms),
            restore_unread_timeout_ms: chosen
                .unread_timeout_ms
                .unwrap_or(configured.restore_unread_timeout_ms),
            restore_after_failed_paste_ms: chosen
                .after_failed_paste_ms
                .unwrap_or(configured.restore_after_failed_paste_ms),
        };
        tuning.validate().map_err(|err| {
            AppError::validation(format!(
                "the chosen paste waits cannot be used: {}",
                err.message
            ))
        })?;
        Ok(tuning)
    }
}
