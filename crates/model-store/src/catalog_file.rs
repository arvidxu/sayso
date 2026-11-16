use std::fmt;

use serde::de::{Error as DeError, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{Value, from_str};

use dictation_types::{
    AppError, AppResult, ArchiveFormat, Language, ModelDescriptor, ModelFile, ModelId, ModelKind,
    PlatformTarget, ProviderId,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogFile {
    models: Vec<ModelEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ModelEntry {
    id: String,
    name: String,
    kind: String,
    engine: String,
    group: Option<String>,
    provided_by_system: bool,
    /// Paths inside the installed folder the engine needs, by name.
    #[serde(default)]
    named_paths: Pairs,
    /// Facts only the model's engine understands, by name. Numbers and true/false are fine.
    #[serde(default)]
    engine_settings: Settings,
    platforms: Vec<String>,
    languages: Vec<String>,
    files: Vec<FileEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FileEntry {
    path: String,
    size_bytes: u64,
    sha256: String,
    download_urls: Vec<String>,
    unpack: Option<String>,
}

/// JSON object entries in written order; a repeated name is kept so it can be refused instead
/// of silently replacing the first value.
#[derive(Default)]
struct Pairs(Vec<(String, String)>);

impl<'de> Deserialize<'de> for Pairs {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct PairsVisitor;

        impl<'de> Visitor<'de> for PairsVisitor {
            type Value = Pairs;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an object of names and paths")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Pairs, A::Error> {
                let mut pairs = Vec::new();
                while let Some(pair) = map.next_entry::<String, String>()? {
                    pairs.push(pair);
                }
                Ok(Pairs(pairs))
            }
        }

        deserializer.deserialize_map(PairsVisitor)
    }
}

/// Like [`Pairs`], with values that may be text, numbers or true/false, kept as text.
#[derive(Default)]
struct Settings(Vec<(String, String)>);

impl<'de> Deserialize<'de> for Settings {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct SettingsVisitor;

        impl<'de> Visitor<'de> for SettingsVisitor {
            type Value = Settings;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an object of names and plain values")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Settings, A::Error> {
                let mut settings = Vec::new();
                while let Some((name, value)) = map.next_entry::<String, Value>()? {
                    let text = match value {
                        Value::String(text) => text,
                        Value::Number(number) => number.to_string(),
                        Value::Bool(flag) => flag.to_string(),
                        _ => {
                            return Err(DeError::custom(format!(
                                "the engine setting '{name}' must be text, a number or true/false"
                            )));
                        }
                    };
                    settings.push((name, text));
                }
                Ok(Settings(settings))
            }
        }

        deserializer.deserialize_map(SettingsVisitor)
    }
}

fn kind_from(text: &str, id: &str) -> AppResult<ModelKind> {
    match text.trim().to_ascii_lowercase().as_str() {
        "asr" => Ok(ModelKind::Asr),
        "llm" => Ok(ModelKind::Llm),
        "vad" => Ok(ModelKind::Vad),
        other => Err(AppError::validation(format!(
            "model '{id}' has an unknown kind '{other}'"
        ))),
    }
}

fn file_from(entry: FileEntry) -> AppResult<ModelFile> {
    let unpack = entry
        .unpack
        .as_deref()
        .map(str::parse::<ArchiveFormat>)
        .transpose()?;
    Ok(ModelFile {
        path: entry.path,
        size_bytes: entry.size_bytes,
        sha256: entry.sha256,
        download_urls: entry.download_urls,
        unpack,
    })
}

fn descriptor_from(entry: ModelEntry) -> AppResult<ModelDescriptor> {
    let where_ = |what: &str, err: AppError| {
        AppError::new(
            err.kind,
            format!("model '{}' {what}: {}", entry.id, err.message),
        )
    };
    let platforms = entry
        .platforms
        .iter()
        .map(|text| text.parse::<PlatformTarget>())
        .collect::<AppResult<Vec<_>>>()
        .map_err(|err| where_("lists a platform that is not known", err))?;
    let languages = entry
        .languages
        .iter()
        .map(|text| Language::parse(text))
        .collect::<AppResult<Vec<_>>>()
        .map_err(|err| where_("lists a language that is not valid", err))?;
    let kind = kind_from(&entry.kind, &entry.id)?;
    let files = entry
        .files
        .into_iter()
        .map(file_from)
        .collect::<AppResult<Vec<_>>>()
        .map_err(|err| where_("lists a file that is not valid", err))?;
    Ok(ModelDescriptor {
        id: ModelId::new(entry.id),
        name: entry.name,
        kind,
        engine: ProviderId::new(entry.engine),
        group: entry.group,
        files,
        provided_by_system: entry.provided_by_system,
        named_paths: entry.named_paths.0,
        engine_settings: entry.engine_settings.0,
        supported_platforms: platforms,
        supported_languages: languages,
    })
}

/// The entries of a catalog file, in the order written, not yet checked against each other.
pub(crate) fn parse(text: &str) -> AppResult<Vec<ModelDescriptor>> {
    let file: CatalogFile = from_str(text)
        .map_err(|err| AppError::validation(format!("the model catalog is not valid: {err}")))?;
    file.models.into_iter().map(descriptor_from).collect()
}
