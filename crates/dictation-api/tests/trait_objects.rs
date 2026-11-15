use dictation_api::{
    AsrEngine, AsrSession, AudioCaptureSession, AudioSource, Clock, ContextProvider,
    DictionaryStore, HistoryStore, HotkeySource, LlmEngine, ModelStore, SecretStore, SettingsStore,
    TextInjector,
};

fn usable_as_object<T: ?Sized>() {}

#[test]
fn every_trait_is_usable_as_a_trait_object() {
    usable_as_object::<dyn AsrSession>();
    usable_as_object::<dyn AsrEngine>();
    usable_as_object::<dyn LlmEngine>();
    usable_as_object::<dyn AudioCaptureSession>();
    usable_as_object::<dyn AudioSource>();
    usable_as_object::<dyn TextInjector>();
    usable_as_object::<dyn HotkeySource>();
    usable_as_object::<dyn ContextProvider>();
    usable_as_object::<dyn HistoryStore>();
    usable_as_object::<dyn DictionaryStore>();
    usable_as_object::<dyn SettingsStore>();
    usable_as_object::<dyn SecretStore>();
    usable_as_object::<dyn ModelStore>();
    usable_as_object::<dyn Clock>();
}
