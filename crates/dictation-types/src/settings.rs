use crate::{
    DeviceId, HotkeyBinding, InsertMethod, Language, Languages, ModeId, ModelId, PasteRestoreWaits,
    ProviderId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OutputTiming {
    WhileSpeaking,
    AtTheEnd,
}

named_values!(OutputTiming, "output timing", {
    WhileSpeaking => "while-speaking",
    AtTheEnd => "at-the-end",
});

/// Only what the dictation flow needs. Nothing is pre-decided: every choice starts `None`,
/// off or empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserSettings {
    /// In order of preference.
    pub languages: Languages,
    pub active_mode_id: Option<ModeId>,
    pub asr_provider_id: Option<ProviderId>,
    pub asr_model_id: Option<ModelId>,
    pub llm_provider_id: Option<ProviderId>,
    pub llm_model_id: Option<ModelId>,
    pub preferred_device_id: Option<DeviceId>,
    /// The microphone to use while a laptop lid is closed.
    pub clamshell_device_id: Option<DeviceId>,
    pub mute_other_audio_while_recording: bool,
    /// `None` means not chosen yet.
    pub insert_method: Option<InsertMethod>,
    pub paste_restore_waits: PasteRestoreWaits,
    /// `None` means not chosen yet.
    pub output_timing: Option<OutputTiming>,
    pub hotkeys: Vec<HotkeyBinding>,
    /// `None` never unloads.
    pub model_idle_unload_secs: Option<u64>,
    /// `None` keeps them until the user deletes them.
    pub audio_retention_days: Option<u32>,
    /// `None` uses the recording package's limit. Not tied to any model: longer audio is cut
    /// into pieces.
    pub max_recording_secs: Option<u32>,
    pub ui_language: Option<Language>,
}

impl UserSettings {
    pub fn new(languages: Languages) -> Self {
        Self {
            languages,
            active_mode_id: None,
            asr_provider_id: None,
            asr_model_id: None,
            llm_provider_id: None,
            llm_model_id: None,
            preferred_device_id: None,
            clamshell_device_id: None,
            mute_other_audio_while_recording: false,
            insert_method: None,
            paste_restore_waits: PasteRestoreWaits::default(),
            output_timing: None,
            hotkeys: Vec::new(),
            model_idle_unload_secs: None,
            audio_retention_days: None,
            max_recording_secs: None,
            ui_language: None,
        }
    }
}
