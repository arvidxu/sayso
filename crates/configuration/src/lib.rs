//! Reads the product configuration: what a new user starts with, and overrides of the
//! configuration each package ships with. A package reads its own values through
//! [`PackageConfig`]; nothing here knows which packages there are.

mod file;
mod overrides;

use dictation_types::{AppResult, InsertMethod, Languages, OutputTiming, UserSettings};

use file::Layer;

/// What a new user's settings start with, before the user changes anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitialSettings {
    /// `None` means a model is never unloaded for being idle.
    pub model_idle_unload_secs: Option<u64>,
    /// `None` means recordings are kept until the user deletes them.
    pub audio_retention_days: Option<u32>,
    /// `None` means the length the recording package is configured with.
    pub max_recording_secs: Option<u32>,
    pub output_timing: OutputTiming,
    pub insert_method: InsertMethod,
    pub mute_other_audio_while_recording: bool,
}

impl InitialSettings {
    /// Settings for a new user who dictates in `languages`. The app supplies the languages; they
    /// never come from the configuration.
    pub fn for_new_user(&self, languages: Languages) -> UserSettings {
        let mut settings = UserSettings::new(languages);
        settings.model_idle_unload_secs = self.model_idle_unload_secs;
        settings.audio_retention_days = self.audio_retention_days;
        settings.max_recording_secs = self.max_recording_secs;
        settings.output_timing = Some(self.output_timing);
        settings.insert_method = Some(self.insert_method);
        settings.mute_other_audio_while_recording = self.mute_other_audio_while_recording;
        settings
    }
}

/// A package's configuration: the file the package ships with, which the product may override
/// value by value under `dictation.overrides.<NAME>`.
pub trait PackageConfig: Sized {
    /// The package's key under `dictation.overrides`.
    const NAME: &'static str;
    /// The configuration file the package ships with.
    const BUNDLED: &'static str;
    /// Reads and checks the package's values. Missing, unknown or out-of-range values fail.
    fn from_json(text: &str) -> AppResult<Self>;
}

/// The `dictation` section of the product configuration: what a new user starts with, and
/// overrides of package configurations, kept until a package reads its own.
#[derive(Debug, Clone, PartialEq)]
pub struct ProductConfig {
    pub initial_settings: InitialSettings,
    layers: Vec<Layer>,
}

impl ProductConfig {
    /// Reads the `dictation` section of the product configuration. Missing, unknown or
    /// mistyped values fail; overrides are checked when a package reads them.
    pub fn from_json(product_config: &str) -> AppResult<Self> {
        Self::from_layers(product_config, &[])
    }

    /// Like [`from_json`](Self::from_json), with further layers shaped like the `overrides`
    /// object (such as a file on the user's machine) laid on top in order and checked the same way.
    pub fn from_layers(product_config: &str, more_overrides: &[&str]) -> AppResult<Self> {
        let (initial_settings, layers) = file::parse(product_config, more_overrides)?;
        Ok(Self {
            initial_settings,
            layers,
        })
    }

    /// The configuration package `T` ships with, with every layer's overrides of it applied
    /// in order and checked by the package.
    pub fn package<T: PackageConfig>(&self) -> AppResult<T> {
        let layers: Vec<_> = self
            .layers
            .iter()
            .filter_map(|layer| layer.packages.get(T::NAME))
            .collect();
        file::package(T::NAME, T::BUNDLED, &layers, T::from_json)
    }

    /// Refuses overrides of a package not in `known`. A package only reads its own
    /// overrides, so one under a misspelt name would otherwise be ignored without a word.
    pub fn check_package_names(&self, known: &[&str]) -> AppResult<()> {
        for layer in &self.layers {
            layer.check_names(known)?;
        }
        Ok(())
    }
}
