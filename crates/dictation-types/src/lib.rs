//! Plain data shared between crates. No dependencies, no product catalogs as constants,
//! and no defaults that decide for the user.

/// For an enum whose values are each written as one name in configuration, stored settings or
/// catalog files: the list of its values, their names, and reading a name back whatever its
/// letter case or the space around it.
macro_rules! named_values {
    ($type:ident, $what:literal, { $($value:ident => $name:literal),+ $(,)? }) => {
        impl $type {
            pub const ALL: [Self; [$($name),+].len()] = [$(Self::$value),+];

            /// The name written in configuration, stored settings and catalog files.
            pub fn as_str(&self) -> &'static str {
                match self {
                    $(Self::$value => $name),+
                }
            }
        }

        impl std::fmt::Display for $type {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl std::str::FromStr for $type {
            type Err = $crate::AppError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let wanted = s.trim().to_ascii_lowercase();
                Self::ALL
                    .into_iter()
                    .find(|value| value.as_str() == wanted)
                    .ok_or_else(|| {
                        $crate::AppError::validation(format!(concat!("unknown ", $what, " '{}'"), s))
                    })
            }
        }
    };
}

mod asr;
mod audio;
mod cancel;
mod context;
mod error;
mod feature;
mod history;
mod hotkey;
mod id;
mod insertion;
mod language;
mod language_data;
mod language_share;
mod languages;
mod llm;
mod mark_data;
mod mode;
mod model;
mod permission;
mod provider;
mod punctuation_data;
mod relative_path;
mod request_route;
mod script_data;
mod segment_joining;
mod settings;
mod stage_timing;
mod term;
mod token_usage;
mod transcript;
mod transcript_segment;

pub use asr::{AsrCaps, AsrOptions, LanguageMixing, ProviderRestriction};
pub use audio::{AudioCaptureConfig, AudioChunk, AudioDevice, InputLevel};
pub use cancel::{CancelToken, CancelWatch};
pub use context::{ActiveAppInfo, AppContext, AppScene, ContextQuery};
pub use error::{AppError, AppResult, ErrorKind, SecretString};
pub use feature::{FeatureDescriptor, FeatureMountPoint};
pub use history::{HistoryEntry, HistoryQuery, HistoryStats};
pub use hotkey::{
    HotkeyBinding, HotkeyNotice, HotkeyTriggerMode, KeyCode, ModifierKey, ShortcutEvent,
    ShortcutState,
};
pub use id::{
    DeviceId, ExtensionId, FeatureId, HistoryId, ModeId, ModelId, ProviderId, RuleId, ShortcutId,
    TermId, TranscriptId,
};
pub use insertion::{InjectionWarning, InsertMethod, InsertOutcome, PasteRead, PasteRestoreWaits};
pub use language::Language;
pub use languages::Languages;
pub use llm::{LlmCaps, LlmChunk, LlmCompletion, LlmMessage, LlmRequest, LlmRole};
pub use mode::{LlmStep, Mode, PromptVariable};
pub use model::{
    ArchiveFormat, DownloadProgress, InstallPhase, ModelDescriptor, ModelFile, ModelInstallState,
    ModelKind, ModelStatus, PlatformTarget, is_valid_sha256_hex,
};
pub use permission::{PermissionKind, PermissionReport, PermissionStatus};
pub use provider::{ProviderCaps, ProviderDescriptor, ProviderDetails, ProviderKind};
pub use relative_path::check_relative_path;
pub use request_route::RequestRoute;
pub use segment_joining::join_segment_texts;
pub use settings::{OutputTiming, UserSettings};
pub use stage_timing::{StageTiming, StageTimings};
pub use term::{FillerWord, ReplacementKind, ReplacementRule, Term, TermKind, fold_case};
pub use token_usage::TokenUsage;
pub use transcript::Transcript;
pub use transcript_segment::TranscriptSegment;
