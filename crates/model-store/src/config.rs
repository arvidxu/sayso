//! Tuning values read from a configuration file; the package ships its own.

use configuration::PackageConfig;
use dictation_types::{AppError, AppResult};
use serde::Deserialize;
use serde_json::{Value, from_str, from_value};

use crate::{DownloadTuning, LocalCacheLocation};

pub const BUNDLED_CONFIG: &str = include_str!("../model-store.conf.json");

#[derive(Debug, Clone, PartialEq)]
pub struct ModelsConfig {
    pub download: DownloadTuning,
    /// Where model files other tools downloaded may already be on this machine.
    pub local_caches: Vec<LocalCacheLocation>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConfigFile {
    download: DownloadFile,
    local_caches: Vec<LocalCacheFile>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DownloadFile {
    connect_timeout_ms: u64,
    read_timeout_ms: u64,
    buffer_bytes: usize,
    attempts_per_address: u32,
    retry_delay_ms: u64,
    unpacked_size_ratio: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LocalCacheFile {
    env_var: Option<String>,
    sub_path: String,
    file_pattern: String,
}

const GROUP_DOWNLOAD: &str = "download";
const GROUP_LOCAL_CACHES: &str = "localCaches";

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

fn in_group(group: &str, err: AppError) -> AppError {
    AppError::new(err.kind, format!("{group}: {}", err.message))
}

impl PackageConfig for ModelsConfig {
    const NAME: &'static str = "model-store";
    const BUNDLED: &'static str = BUNDLED_CONFIG;

    fn from_json(text: &str) -> AppResult<Self> {
        Self::from_json(text)
    }
}

impl ModelsConfig {
    pub fn bundled() -> AppResult<Self> {
        Self::from_json(BUNDLED_CONFIG)
    }

    /// Anything missing, unknown or out of range is an error.
    pub fn from_json(text: &str) -> AppResult<Self> {
        let invalid =
            |err| AppError::validation(format!("the models configuration is not valid: {err}"));
        let value: Value = from_str(text).map_err(invalid)?;
        named_values(&value, "the models configuration")?;
        // Overrides replace a list whole, so its entries can arrive written as lists of values.
        if let Some(Value::Array(caches)) = value.get(GROUP_LOCAL_CACHES) {
            for cache in caches {
                named_values(cache, GROUP_LOCAL_CACHES)?;
            }
        }
        let file: ConfigFile = from_value(value).map_err(invalid)?;
        let config = Self {
            download: DownloadTuning {
                connect_timeout_ms: file.download.connect_timeout_ms,
                read_timeout_ms: file.download.read_timeout_ms,
                buffer_bytes: file.download.buffer_bytes,
                attempts_per_address: file.download.attempts_per_address,
                retry_delay_ms: file.download.retry_delay_ms,
                unpacked_size_ratio: file.download.unpacked_size_ratio,
            },
            local_caches: file
                .local_caches
                .into_iter()
                .map(|cache| LocalCacheLocation {
                    env_var: cache.env_var,
                    sub_path: cache.sub_path,
                    file_pattern: cache.file_pattern,
                })
                .collect(),
        };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> AppResult<()> {
        self.download
            .validate()
            .map_err(|err| in_group(GROUP_DOWNLOAD, err))?;
        for cache in &self.local_caches {
            cache
                .validate()
                .map_err(|err| in_group(GROUP_LOCAL_CACHES, err))?;
        }
        Ok(())
    }
}
