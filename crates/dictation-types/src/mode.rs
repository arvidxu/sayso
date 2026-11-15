use crate::{AppError, AppResult, InsertMethod, Language, ModeId};

/// Written `{name}` in a template; only these are supplied by the flow, see
/// [`Mode::placeholders`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PromptVariable {
    /// The recognised speech.
    Text,
    SelectedText,
    ClipboardText,
    AppScene,
    Languages,
    TargetLanguage,
    DictionaryTerms,
}

impl PromptVariable {
    pub const ALL: [Self; 7] = [
        Self::Text,
        Self::SelectedText,
        Self::ClipboardText,
        Self::AppScene,
        Self::Languages,
        Self::TargetLanguage,
        Self::DictionaryTerms,
    ];

    /// The name used between braces in a template.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::SelectedText => "selected",
            Self::ClipboardText => "clipboard",
            Self::AppScene => "scene",
            Self::Languages => "languages",
            Self::TargetLanguage => "target_language",
            Self::DictionaryTerms => "terms",
        }
    }

    /// For example `{text}`.
    pub fn placeholder(&self) -> String {
        format!("{{{}}}", self.name())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmStep {
    pub prompt_template: String,
    /// Input shorter than this many characters skips the language model.
    pub skip_under_chars: Option<usize>,
}

/// What happens to recognised speech before it is inserted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mode {
    pub id: ModeId,
    pub name: String,
    /// `None` applies text rules only.
    pub llm: Option<LlmStep>,
    /// `None` uses the user's setting.
    pub insert_method: Option<InsertMethod>,
    pub target_language: Option<Language>,
}

impl Mode {
    pub fn without_llm(id: ModeId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            llm: None,
            insert_method: None,
            target_language: None,
        }
    }

    pub fn with_llm(
        id: ModeId,
        name: impl Into<String>,
        prompt_template: impl Into<String>,
    ) -> Self {
        Self {
            llm: Some(LlmStep {
                prompt_template: prompt_template.into(),
                skip_under_chars: None,
            }),
            ..Self::without_llm(id, name)
        }
    }

    pub fn validate(&self) -> AppResult<()> {
        if self.id.is_empty() {
            return Err(AppError::validation("mode ID must not be empty"));
        }
        if self.name.trim().is_empty() {
            return Err(AppError::validation("mode name must not be empty"));
        }
        if let Some(step) = &self.llm
            && step.prompt_template.trim().is_empty()
        {
            return Err(AppError::validation(format!(
                "mode '{}' uses a language model but has no prompt",
                self.id
            )));
        }
        Ok(())
    }

    /// In order of first appearance; names are letters, digits and underscores in braces.
    pub fn placeholders(&self) -> Vec<&str> {
        let Some(step) = &self.llm else {
            return Vec::new();
        };
        let mut names: Vec<&str> = Vec::new();
        let mut rest = step.prompt_template.as_str();
        while let Some(open) = rest.find('{') {
            rest = &rest[open + 1..];
            let Some(close) = rest.find('}') else { break };
            let name = &rest[..close];
            let is_name =
                !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
            if is_name {
                if !names.contains(&name) {
                    names.push(name);
                }
                rest = &rest[close + 1..];
            }
        }
        names
    }

    pub fn uses(&self, variable: PromptVariable) -> bool {
        self.placeholders().contains(&variable.name())
    }

    pub fn should_run_llm(&self, char_count: usize) -> bool {
        self.llm
            .as_ref()
            .is_some_and(|step| step.skip_under_chars.is_none_or(|min| char_count >= min))
    }
}
