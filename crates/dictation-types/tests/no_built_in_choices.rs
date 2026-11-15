mod support;

use dictation_types::{
    AsrCaps, AsrOptions, FillerWord, HistoryEntry, Language, Languages, LlmRequest, Mode, ModeId,
    ModelDescriptor, ModelId, PasteRestoreWaits, ProviderDescriptor, ProviderId, Term, Transcript,
    UserSettings,
};
use support::langs;

/// Fails to compile if a listed type implements `Default`: the trait lookup becomes ambiguous.
macro_rules! assert_cannot_be_defaulted {
    ($($ty:ty),+ $(,)?) => {$(
        const _: fn() = || {
            trait AmbiguousIfDefault<Marker> {
                fn check() {}
            }
            impl<T: ?Sized> AmbiguousIfDefault<()> for T {}
            #[allow(dead_code)]
            struct HasDefault;
            impl<T: ?Sized + Default> AmbiguousIfDefault<HasDefault> for T {}
            let _ = <$ty as AmbiguousIfDefault<_>>::check;
        };
    )+};
}

// These exist only once the caller supplies the languages or the name.
assert_cannot_be_defaulted!(
    Language,
    Languages,
    UserSettings,
    AsrCaps,
    AsrOptions,
    Transcript,
    HistoryEntry,
    Term,
    FillerWord,
    Mode,
    ModeId,
    ModelId,
    ProviderId,
    ModelDescriptor,
    ProviderDescriptor,
    LlmRequest,
);

#[test]
fn new_settings_choose_no_mode_provider_model_or_device() {
    let settings = UserSettings::new(langs("pt+it"));
    assert_eq!(settings.active_mode_id, None);
    assert_eq!(settings.asr_provider_id, None);
    assert_eq!(settings.asr_model_id, None);
    assert_eq!(settings.llm_provider_id, None);
    assert_eq!(settings.llm_model_id, None);
    assert_eq!(settings.preferred_device_id, None);
    assert_eq!(settings.clamshell_device_id, None);
    assert_eq!(settings.ui_language, None);
    assert_eq!(settings.model_idle_unload_secs, None);
    assert_eq!(settings.audio_retention_days, None);
    assert_eq!(settings.max_recording_secs, None);
    assert_eq!(settings.insert_method, None);
    assert_eq!(settings.output_timing, None);
    assert_eq!(settings.paste_restore_waits, PasteRestoreWaits::default());
    assert!(settings.hotkeys.is_empty());
}
