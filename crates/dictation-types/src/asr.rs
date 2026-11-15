use crate::{AppError, AppResult, Language, Languages, ModelId, Term};

/// A limit an engine declares, or a way a request exceeds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderRestriction {
    StreamingUnsupported,
    BatchUnsupported,
    /// Most seconds of audio taken at once; longer recordings are cut into pieces, not refused.
    MaxDurationSecs(u32),
    HotwordsUnsupported,
    UnsupportedLanguages(Vec<Language>),
    LanguageMixingUnsupported,
    OneLanguagePerUtteranceAutoDetected,
    LanguageMixingLimitedToGroups(Vec<Vec<Language>>),
    /// As a limit, declared regardless of the request; against a request, too many were asked for.
    TooManyLanguages {
        max: usize,
    },
    LanguageCombinationUnsupported,
    PerSegmentLanguageUnsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LanguageMixing {
    /// The engine has to be told which language.
    OnePerUtterance,
    AutoDetectOne,
    /// `None` means any number of languages.
    Mixed {
        max_languages: Option<usize>,
    },
    MixedWithinGroups(Vec<Vec<Language>>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsrCaps {
    pub supports_streaming: bool,
    pub supports_batch: bool,
    /// Seconds of audio taken at once; not a limit on recording length.
    pub max_duration_secs: Option<u32>,
    pub supports_hotwords: bool,
    /// A language here covers all its regional and script variants.
    pub supported_languages: Vec<Language>,
    pub language_mixing: LanguageMixing,
    pub supports_per_segment_language: bool,
    pub operates_locally: bool,
    /// The engine expects mono audio at this rate.
    pub sample_rate_hz: u32,
}

impl AsrCaps {
    /// Batch only, one language per utterance, no hotwords, no per-segment language;
    /// set the fields for anything more.
    pub fn new(
        supported_languages: Vec<Language>,
        sample_rate_hz: u32,
        operates_locally: bool,
    ) -> Self {
        Self {
            supports_streaming: false,
            supports_batch: true,
            max_duration_secs: None,
            supports_hotwords: false,
            supported_languages,
            language_mixing: LanguageMixing::OnePerUtterance,
            supports_per_segment_language: false,
            operates_locally,
            sample_rate_hz,
        }
    }

    /// Any regional or script variant counts.
    pub fn supports_language(&self, language: &Language) -> bool {
        self.supported_languages.iter().any(|l| l.matches(language))
    }

    /// Why the engine cannot take an utterance in `languages`; empty when it can.
    pub fn check_languages(&self, languages: &Languages) -> Vec<ProviderRestriction> {
        let unsupported: Vec<Language> = languages
            .iter()
            .filter(|l| !self.supports_language(l))
            .cloned()
            .collect();
        if !unsupported.is_empty() {
            return vec![ProviderRestriction::UnsupportedLanguages(unsupported)];
        }

        let count = languages.distinct_languages();
        if count <= 1 {
            return Vec::new();
        }
        match &self.language_mixing {
            LanguageMixing::OnePerUtterance => {
                vec![ProviderRestriction::LanguageMixingUnsupported]
            }
            LanguageMixing::AutoDetectOne => Vec::new(),
            LanguageMixing::Mixed { max_languages } => match max_languages {
                Some(max) if count > *max => {
                    vec![ProviderRestriction::TooManyLanguages { max: *max }]
                }
                _ => Vec::new(),
            },
            LanguageMixing::MixedWithinGroups(groups) => {
                let fits = groups.iter().any(|group| {
                    languages
                        .iter()
                        .all(|l| group.iter().any(|member| member.matches(l)))
                });
                if fits {
                    Vec::new()
                } else {
                    vec![ProviderRestriction::LanguageCombinationUnsupported]
                }
            }
        }
    }

    pub fn supports_languages(&self, languages: &Languages) -> bool {
        self.check_languages(languages).is_empty()
    }

    pub fn validate(&self) -> AppResult<()> {
        if !self.supports_streaming && !self.supports_batch {
            return Err(AppError::validation(
                "ASR capabilities must support at least one of streaming or batch recognition",
            ));
        }
        if self.max_duration_secs == Some(0) {
            return Err(AppError::validation(
                "ASR max_duration_secs must be greater than 0 when specified",
            ));
        }
        if self.supported_languages.is_empty() {
            return Err(AppError::validation(
                "ASR supported_languages must not be empty",
            ));
        }
        if self.sample_rate_hz == 0 {
            return Err(AppError::validation(
                "ASR sample_rate_hz must be greater than 0",
            ));
        }
        match &self.language_mixing {
            LanguageMixing::Mixed {
                max_languages: Some(max),
            } if *max < 2 => Err(AppError::validation(
                "an engine that mixes languages must allow at least 2",
            )),
            LanguageMixing::MixedWithinGroups(groups)
                if groups.is_empty() || groups.iter().any(|group| group.len() < 2) =>
            {
                Err(AppError::validation(
                    "language mixing groups must exist and hold at least 2 languages each",
                ))
            }
            LanguageMixing::MixedWithinGroups(groups)
                if groups.iter().flatten().any(|l| !self.supports_language(l)) =>
            {
                Err(AppError::validation(
                    "a language mixing group contains a language that is not supported",
                ))
            }
            _ => Ok(()),
        }
    }

    pub fn exceeds_max_duration(&self, duration_secs: u32) -> bool {
        self.max_duration_secs
            .is_some_and(|max_secs| duration_secs > max_secs)
    }

    /// Limits declared regardless of any request, for display; see [`Self::check_options`].
    pub fn limits(&self) -> Vec<ProviderRestriction> {
        let mut list = Vec::new();
        if !self.supports_streaming {
            list.push(ProviderRestriction::StreamingUnsupported);
        }
        if !self.supports_batch {
            list.push(ProviderRestriction::BatchUnsupported);
        }
        if let Some(max_secs) = self.max_duration_secs {
            list.push(ProviderRestriction::MaxDurationSecs(max_secs));
        }
        if !self.supports_hotwords {
            list.push(ProviderRestriction::HotwordsUnsupported);
        }
        match &self.language_mixing {
            LanguageMixing::OnePerUtterance => {
                list.push(ProviderRestriction::LanguageMixingUnsupported);
            }
            LanguageMixing::AutoDetectOne => {
                list.push(ProviderRestriction::OneLanguagePerUtteranceAutoDetected);
            }
            LanguageMixing::Mixed {
                max_languages: Some(max),
            } => list.push(ProviderRestriction::TooManyLanguages { max: *max }),
            LanguageMixing::Mixed {
                max_languages: None,
            } => {}
            LanguageMixing::MixedWithinGroups(groups) => {
                list.push(ProviderRestriction::LanguageMixingLimitedToGroups(
                    groups.clone(),
                ));
            }
        }
        if !self.supports_per_segment_language {
            list.push(ProviderRestriction::PerSegmentLanguageUnsupported);
        }
        list
    }

    /// What the engine cannot do of `opts` and the recording length, if known; empty when
    /// within its limits.
    pub fn check_options(
        &self,
        opts: &AsrOptions,
        duration_secs: Option<u32>,
    ) -> Vec<ProviderRestriction> {
        let mut violations = Vec::new();
        if opts.streaming && !self.supports_streaming {
            violations.push(ProviderRestriction::StreamingUnsupported);
        }
        if !opts.streaming && !self.supports_batch {
            violations.push(ProviderRestriction::BatchUnsupported);
        }
        if let (Some(duration), Some(max_secs)) = (duration_secs, self.max_duration_secs)
            && duration > max_secs
        {
            violations.push(ProviderRestriction::MaxDurationSecs(max_secs));
        }
        if !opts.hotwords.is_empty() && !self.supports_hotwords {
            violations.push(ProviderRestriction::HotwordsUnsupported);
        }
        if opts.per_segment_language && !self.supports_per_segment_language {
            violations.push(ProviderRestriction::PerSegmentLanguageUnsupported);
        }
        violations.extend(self.check_languages(&opts.languages));
        violations
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AsrOptions {
    pub model_id: Option<ModelId>,
    /// In order of preference.
    pub languages: Languages,
    pub streaming: bool,
    pub hotwords: Vec<Term>,
    pub per_segment_language: bool,
}

impl AsrOptions {
    /// Batch, with nothing optional requested.
    pub fn new(languages: Languages) -> Self {
        Self {
            model_id: None,
            languages,
            streaming: false,
            hotwords: Vec::new(),
            per_segment_language: false,
        }
    }
}
