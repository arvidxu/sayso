use std::num::NonZero;
use std::thread::available_parallelism;

use configuration::PackageConfig;
use dictation_types::{AppError, AppResult};
use serde::Deserialize;
use serde_json::{Value, from_str, from_value};

pub const BUNDLED_CONFIG: &str = include_str!("../asr-local.conf.json");

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConfigFile {
    thread_share: f32,
    execution_provider: String,
    load_timeout_ms: u64,
    worker_parent_check_ms: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecognitionTuning {
    /// Share of logical processors, in (0, 1]. More is not always faster: slow cores hold
    /// fast ones back.
    pub thread_share: f32,
    /// The engine's own name for the compute device (processor, graphics chip).
    pub execution_provider: String,
    /// Only engines that can be stopped mid-load, such as one in a worker process, honour it.
    pub load_timeout_ms: u64,
    /// How often a worker checks its parent is alive, so a worker stuck in the engine does
    /// not outlive it.
    pub worker_parent_check_ms: u64,
}

impl PackageConfig for RecognitionTuning {
    const NAME: &'static str = "asr-local";
    const BUNDLED: &'static str = BUNDLED_CONFIG;

    fn from_json(text: &str) -> AppResult<Self> {
        Self::from_json(text)
    }
}

impl RecognitionTuning {
    pub fn bundled() -> AppResult<Self> {
        Self::from_json(BUNDLED_CONFIG)
    }

    /// Every value is required; missing, unknown or out-of-range values are errors.
    pub fn from_json(text: &str) -> AppResult<Self> {
        let invalid = |err| {
            AppError::validation(format!("the recognition configuration is not valid: {err}"))
        };
        let value: Value = from_str(text).map_err(invalid)?;
        if !value.is_object() {
            return Err(AppError::validation(
                "the recognition configuration must be an object with a name for every value",
            ));
        }
        let file: ConfigFile = from_value(value).map_err(invalid)?;
        let tuning = Self {
            thread_share: file.thread_share,
            execution_provider: file.execution_provider,
            load_timeout_ms: file.load_timeout_ms,
            worker_parent_check_ms: file.worker_parent_check_ms,
        };
        tuning.validate()?;
        Ok(tuning)
    }

    pub fn validate(&self) -> AppResult<()> {
        if !(self.thread_share > 0.0 && self.thread_share <= 1.0) {
            return Err(AppError::validation(
                "thread_share must be above 0 and at most 1",
            ));
        }
        let provider = &self.execution_provider;
        if provider.trim().is_empty() || provider.chars().any(char::is_control) {
            return Err(AppError::validation(
                "execution_provider must be a plain, non-empty name",
            ));
        }
        if self.load_timeout_ms == 0 {
            return Err(AppError::validation(
                "load_timeout_ms must be greater than zero",
            ));
        }
        if self.worker_parent_check_ms == 0 {
            return Err(AppError::validation(
                "worker_parent_check_ms must be greater than zero",
            ));
        }
        Ok(())
    }

    /// At least one.
    pub fn threads(&self) -> usize {
        let processors = available_parallelism().map_or(1, NonZero::get);
        ((processors as f32 * self.thread_share).round() as usize).clamp(1, processors)
    }
}
