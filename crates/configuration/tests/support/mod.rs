#![allow(dead_code)]

//! Two made-up packages, so that the tests check how overrides are read without depending
//! on any real package.

use configuration::{PackageConfig, ProductConfig};
use dictation_types::{AppError, AppResult, ErrorKind};
use serde::Deserialize;
use serde_json::{Value, from_str};

pub const PRODUCT_FILE: &str = include_str!("../../../../product.conf.json");

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Sample {
    pub timing: Timing,
    pub share: f64,
    pub label: String,
    pub fallback: Option<String>,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Timing {
    pub timeout_ms: u64,
    pub window_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Item {
    pub path: String,
    pub weight: u32,
}

impl PackageConfig for Sample {
    const NAME: &'static str = "sample";
    const BUNDLED: &'static str = r#"{
        "timing": { "timeoutMs": 30, "windowMs": 10 },
        "share": 0.5,
        "label": "one",
        "fallback": null,
        "items": [ { "path": "a", "weight": 1 }, { "path": "b", "weight": 2 } ]
    }"#;

    fn from_json(text: &str) -> AppResult<Self> {
        let config: Self = from_str(text).map_err(|err| {
            AppError::validation(format!("the sample configuration is not valid: {err}"))
        })?;
        if config.timing.timeout_ms == 0 {
            return Err(AppError::validation("timing.timeoutMs must be above 0"));
        }
        if config.timing.window_ms > config.timing.timeout_ms {
            return Err(AppError::validation(
                "timing.windowMs must not be longer than timing.timeoutMs",
            ));
        }
        if !(config.share > 0.0 && config.share <= 1.0) {
            return Err(AppError::validation("share must be in (0, 1]"));
        }
        Ok(config)
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Other {
    pub level: u32,
}

impl PackageConfig for Other {
    const NAME: &'static str = "other";
    const BUNDLED: &'static str = r#"{ "level": 3 }"#;

    fn from_json(text: &str) -> AppResult<Self> {
        let config: Self = from_str(text).map_err(|err| {
            AppError::validation(format!("the other configuration is not valid: {err}"))
        })?;
        if config.level == 0 {
            return Err(AppError::validation("level must be above 0"));
        }
        Ok(config)
    }
}

pub const KNOWN: [&str; 2] = [Sample::NAME, Other::NAME];

/// The product file with `overrides` as its own overrides.
pub fn product_with(overrides: Value) -> String {
    let mut file: Value = from_str(PRODUCT_FILE).expect("JSON");
    file["dictation"]["overrides"] = overrides;
    file.to_string()
}

/// Package `T` read from the product file with `overrides` and the further `layers`.
pub fn read<T: PackageConfig>(overrides: Value, layers: &[&str]) -> AppResult<T> {
    ProductConfig::from_layers(&product_with(overrides), layers)?.package::<T>()
}

/// What package `T` ships with.
pub fn bundled<T: PackageConfig>() -> T {
    T::from_json(T::BUNDLED).expect("the made-up package's own values are valid")
}

/// The message reading package `T` with `overrides` and `layers` fails with.
pub fn refused<T: PackageConfig + std::fmt::Debug>(overrides: Value, layers: &[&str]) -> String {
    match read::<T>(overrides.clone(), layers) {
        Ok(config) => panic!("{overrides} {layers:?} was accepted as {config:?}"),
        Err(err) => {
            assert_eq!(err.kind, ErrorKind::Validation, "{overrides}: {err}");
            err.to_string()
        }
    }
}
