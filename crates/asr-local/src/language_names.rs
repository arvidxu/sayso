use dictation_types::{AppError, AppResult, AsrOptions, Language, ModelDescriptor};

/// For example `languageName.de`. A language without such a setting cannot be hinted.
pub const LANGUAGE_NAME_PREFIX: &str = "languageName.";

/// The model's own name for each language it can be hinted with.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LanguageNames {
    names: Vec<(Language, String)>,
}

impl LanguageNames {
    /// A setting whose code is not a language tag is an error; skipping it would silently
    /// drop the hint.
    pub fn of(model: &ModelDescriptor) -> AppResult<Self> {
        let mut names = Vec::new();
        for (setting, name) in &model.engine_settings {
            let Some(code) = setting.strip_prefix(LANGUAGE_NAME_PREFIX) else {
                continue;
            };
            let language = Language::parse(code).map_err(|err| {
                AppError::model(format!(
                    "model {}: the engine setting '{setting}' does not name a language: {}",
                    model.id, err.message
                ))
            })?;
            names.push((language, name.clone()));
        }
        Ok(Self { names })
    }

    /// Only for a single-language speaker: a hint would stop the model following language
    /// switches. Matched by canonical code, so `zh-yue` is not `zh` and `in` finds `id`.
    pub fn hint_for(&self, opts: &AsrOptions) -> Option<&str> {
        if opts.languages.distinct_languages() != 1 {
            return None;
        }
        let wanted = opts.languages.primary().canonical_primary();
        self.names
            .iter()
            .find(|(language, _)| language.canonical_primary() == wanted)
            .map(|(_, name)| name.as_str())
    }
}
