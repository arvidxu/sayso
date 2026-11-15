use dictation_types::{AppError, AppResult, InsertMethod, OutputTiming};
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Value, from_str};

use crate::InitialSettings;
use crate::overrides::{Strict, apply};

#[derive(Deserialize)]
struct ProductFile {
    dictation: DictationFile,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DictationFile {
    initial_settings: InitialSettingsFile,
    overrides: Strict,
}

/// One object of overrides of single values in the configuration a package ships with, by
/// package name. A package that is not named runs with its own values.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Layer {
    /// Where the layer was written, for error messages.
    place: String,
    pub(crate) packages: Map<String, Value>,
}

impl Layer {
    fn new(place: String, overrides: Value) -> AppResult<Self> {
        let Value::Object(object) = overrides else {
            return Err(AppError::validation(format!(
                "{place} must be an object with a package name for every override"
            )));
        };
        let mut packages = Map::new();
        for (name, value) in object {
            match value {
                // Written out as null, a package has no overrides.
                Value::Null => {}
                Value::Object(_) => {
                    packages.insert(name, value);
                }
                _ => {
                    return Err(AppError::validation(format!(
                        "{place}.{name} must be an object"
                    )));
                }
            }
        }
        Ok(Self { place, packages })
    }

    pub(crate) fn check_names(&self, known: &[&str]) -> AppResult<()> {
        match self
            .packages
            .keys()
            .find(|name| !known.contains(&name.as_str()))
        {
            Some(name) => Err(AppError::validation(format!(
                "{}.{name} overrides a package there is none of",
                self.place
            ))),
            None => Ok(()),
        }
    }
}

/// Reads a value that has to be written out, even when it is `null`.
fn nullable<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(deserializer)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InitialSettingsFile {
    #[serde(deserialize_with = "nullable")]
    model_idle_unload_secs: Option<u64>,
    #[serde(deserialize_with = "nullable")]
    audio_retention_days: Option<u32>,
    #[serde(deserialize_with = "nullable")]
    max_recording_secs: Option<u32>,
    output_timing: String,
    insert_method: String,
    mute_other_audio_while_recording: bool,
}

/// `err` with the place in the product file it is about named in front.
fn at(place: &str, err: AppError) -> AppError {
    AppError::new(err.kind, format!("{place}: {}", err.message))
}

/// Reads one package's configuration with its overrides applied. Errors name the overrides
/// when there are any, and the package's own file otherwise.
pub(crate) fn package<T>(
    name: &str,
    bundled: &str,
    layers: &[&Value],
    read: impl FnOnce(&str) -> AppResult<T>,
) -> AppResult<T> {
    let place = if layers.is_empty() {
        format!("the configuration the {name} package ships with")
    } else {
        format!("dictation.overrides.{name}")
    };
    let text = apply(name, bundled, layers)?;
    read(&text).map_err(|err| at(&place, err))
}

fn initial_settings(file: InitialSettingsFile) -> AppResult<InitialSettings> {
    for (key, is_zero) in [
        (
            "modelIdleUnloadSecs",
            file.model_idle_unload_secs == Some(0),
        ),
        ("audioRetentionDays", file.audio_retention_days == Some(0)),
        ("maxRecordingSecs", file.max_recording_secs == Some(0)),
    ] {
        if is_zero {
            return Err(AppError::validation(format!(
                "{key} must be above 0, or null to leave it open"
            )));
        }
    }
    Ok(InitialSettings {
        model_idle_unload_secs: file.model_idle_unload_secs,
        audio_retention_days: file.audio_retention_days,
        max_recording_secs: file.max_recording_secs,
        output_timing: file.output_timing.parse::<OutputTiming>()?,
        insert_method: file.insert_method.parse::<InsertMethod>()?,
        mute_other_audio_while_recording: file.mute_other_audio_while_recording,
    })
}

pub(crate) fn parse(
    product_config: &str,
    more_overrides: &[&str],
) -> AppResult<(InitialSettings, Vec<Layer>)> {
    let invalid = |err| {
        AppError::validation(format!(
            "the dictation section of the product configuration is not valid: {err}"
        ))
    };
    let file: ProductFile = from_str(product_config).map_err(invalid)?;
    let dictation = file.dictation;
    let mut layers = vec![Layer::new(
        "dictation.overrides".to_owned(),
        dictation.overrides.0,
    )?];
    for (position, text) in more_overrides.iter().enumerate() {
        let place = format!("the additional overrides (number {})", position + 1);
        let invalid = |err| AppError::validation(format!("{place} are not valid: {err}"));
        let overrides = from_str::<Strict>(text).map_err(invalid)?;
        layers.push(Layer::new(place, overrides.0)?);
    }
    let initial_settings = initial_settings(dictation.initial_settings)
        .map_err(|err| at("dictation.initialSettings", err))?;
    Ok((initial_settings, layers))
}
