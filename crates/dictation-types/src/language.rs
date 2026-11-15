use std::fmt;
use std::str::FromStr;

use crate::language_data;
use crate::{AppError, AppResult};

/// A language tag, stored normalized (`pt_br` equals `PT-BR`). Use [`Language::matches`] to
/// compare languages regardless of region, script or alias code.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Language {
    language: String,
    extlangs: Vec<String>,
    script: Option<String>,
    region: Option<String>,
    variants: Vec<String>,
}

impl Language {
    /// Accepts `-` or `_` and any case. A script after the region, or an empty, over-long or
    /// non-alphanumeric subtag, is invalid.
    pub fn parse(raw: &str) -> AppResult<Self> {
        let trimmed = raw.trim();
        let invalid =
            |why: &str| AppError::validation(format!("'{trimmed}' is not a language tag: {why}"));

        let mut subtags = trimmed.split(['-', '_']);
        let primary = subtags.next().unwrap_or_default().to_ascii_lowercase();
        if !(2..=8).contains(&primary.len()) || !primary.bytes().all(|b| b.is_ascii_alphabetic()) {
            return Err(invalid(
                "it must start with a language code of 2 to 8 letters",
            ));
        }

        let mut language = Self {
            language: primary,
            extlangs: Vec::new(),
            script: None,
            region: None,
            variants: Vec::new(),
        };
        // After a single-character subtag, the rest belongs to that extension and is kept as given.
        let mut in_extension = false;
        for subtag in subtags {
            if subtag.is_empty()
                || subtag.len() > 8
                || !subtag.bytes().all(|b| b.is_ascii_alphanumeric())
            {
                return Err(invalid(&format!("'{subtag}' is not a valid subtag")));
            }
            if in_extension || subtag.len() == 1 {
                in_extension = true;
                language.variants.push(subtag.to_ascii_lowercase());
                continue;
            }
            let is_alpha = subtag.bytes().all(|b| b.is_ascii_alphabetic());
            let is_digit = subtag.bytes().all(|b| b.is_ascii_digit());
            let before_script = language.script.is_none() && before_region(&language);

            if subtag.len() == 3 && is_alpha && before_script && language.extlangs.len() < 3 {
                language.extlangs.push(subtag.to_ascii_lowercase());
            } else if subtag.len() == 4 && is_alpha {
                if !before_script {
                    return Err(invalid(&format!(
                        "the script '{subtag}' must come before the region"
                    )));
                }
                let mut script = subtag.to_ascii_lowercase();
                script[..1].make_ascii_uppercase();
                language.script = Some(script);
            } else if before_region(&language)
                && ((subtag.len() == 2 && is_alpha) || (subtag.len() == 3 && is_digit))
            {
                language.region = Some(subtag.to_ascii_uppercase());
            } else {
                language.variants.push(subtag.to_ascii_lowercase());
            }
        }
        Ok(language)
    }

    /// The normalized tag, for example `zh-Hant-HK`.
    pub fn code(&self) -> String {
        let mut code = self.language.clone();
        for part in self
            .extlangs
            .iter()
            .chain(self.script.iter())
            .chain(self.region.iter())
            .chain(self.variants.iter())
        {
            code.push('-');
            code.push_str(part);
        }
        code
    }

    /// The language code without script or region, for example `zh` for `zh-Hant-HK`.
    pub fn primary(&self) -> &str {
        &self.language
    }

    pub fn script_subtag(&self) -> Option<&str> {
        self.script.as_deref()
    }

    pub fn region(&self) -> Option<&str> {
        self.region.as_deref()
    }

    /// Same language whatever script or region (`pt` matches `pt-BR`), including replaced codes
    /// (`iw`, `he`) and macrolanguage codes (`no`, `nb`).
    pub fn matches(&self, other: &Self) -> bool {
        self.canonical_primary() == other.canonical_primary()
    }

    /// The extended subtag if any (`yue` for `zh-yue`), else the primary, mapped to its current
    /// or macrolanguage code (`ar` for `arb`).
    pub fn canonical_primary(&self) -> &str {
        let code = self.extlangs.first().unwrap_or(&self.language);
        language_data::canonical_code(code)
    }

    /// For engines and catalogs: primary replaced by [`Self::canonical_primary`], extended
    /// subtag dropped (`he-IL` for `iw-IL`).
    pub fn canonical(&self) -> Self {
        Self {
            language: self.canonical_primary().to_owned(),
            extlangs: Vec::new(),
            ..self.clone()
        }
    }
}

fn before_region(language: &Language) -> bool {
    language.region.is_none() && language.variants.is_empty()
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.code())
    }
}

impl FromStr for Language {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}
