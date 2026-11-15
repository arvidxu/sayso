use dictation_types::{
    AsrCaps, HotkeyBinding, HotkeyTriggerMode, KeyCode, Language, LanguageMixing, ModelDescriptor,
    ModelFile, ModelId, ModelKind, ModifierKey, PlatformTarget, ProviderId, ShortcutId,
    TranscriptSegment,
};

const SHA: &str = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";

fn file(path: &str) -> ModelFile {
    ModelFile {
        path: path.to_owned(),
        size_bytes: 10,
        sha256: SHA.to_owned(),
        download_urls: vec!["https://example.invalid/file.bin".to_owned()],
        unpack: None,
    }
}

fn model(files: Vec<ModelFile>) -> ModelDescriptor {
    ModelDescriptor {
        id: ModelId::new("m"),
        name: "M".to_owned(),
        kind: ModelKind::Asr,
        engine: ProviderId::new("test-engine"),
        group: None,
        files,
        provided_by_system: false,
        named_paths: Vec::new(),
        engine_settings: Vec::new(),
        supported_platforms: vec![PlatformTarget::LinuxX64],
        supported_languages: vec![Language::parse("pt").unwrap()],
    }
}

#[test]
fn a_download_source_has_to_look_like_an_address() {
    for bad in [
        "not a url",
        "example.invalid/file",
        "https://",
        "://x",
        "https://a b",
    ] {
        let mut f = file("m.bin");
        f.download_urls = vec![bad.to_owned()];
        assert!(f.validate().is_err(), "{bad:?}");
    }
}

#[test]
fn files_that_differ_only_by_letter_case_are_refused() {
    assert!(
        model(vec![file("Model.bin"), file("model.bin")])
            .validate()
            .is_err()
    );
    model(vec![file("model.bin"), file("tokens.txt")])
        .validate()
        .expect("distinct files");
}

#[test]
fn a_catalog_entry_cannot_contradict_itself() {
    let mut system_with_files = model(vec![file("m.bin")]);
    system_with_files.provided_by_system = true;
    assert!(system_with_files.validate().is_err());

    let mut twice = model(vec![file("m.bin")]);
    twice.supported_platforms = vec![PlatformTarget::LinuxX64, PlatformTarget::LinuxX64];
    assert!(twice.validate().is_err());
}

#[test]
fn language_mixing_groups_have_to_allow_some_mixing() {
    let langs = vec![
        Language::parse("pt").unwrap(),
        Language::parse("es").unwrap(),
    ];
    let mut caps = AsrCaps::new(langs.clone(), 16_000, true);
    caps.language_mixing = LanguageMixing::MixedWithinGroups(Vec::new());
    assert!(caps.validate().is_err(), "no groups");
    caps.language_mixing = LanguageMixing::MixedWithinGroups(vec![vec![langs[0].clone()]]);
    assert!(caps.validate().is_err(), "a group of one");
    caps.language_mixing = LanguageMixing::MixedWithinGroups(vec![langs]);
    caps.validate().expect("a real group");
}

#[test]
fn a_confidence_that_is_not_a_number_is_dropped() {
    let segment = TranscriptSegment::new("x", 0, 1)
        .unwrap()
        .with_confidence(f32::NAN);
    assert_eq!(segment.confidence, None);
}

fn combo(modifiers: Vec<ModifierKey>, key: KeyCode, mode: HotkeyTriggerMode) -> HotkeyBinding {
    HotkeyBinding::combination(ShortcutId::new("s"), modifiers, key, mode)
}

#[test]
fn a_shortcut_has_to_be_usable_as_a_global_shortcut() {
    let hold = HotkeyTriggerMode::HoldToTalk;
    let ctrl = vec![ModifierKey::LeftControl];

    assert!(
        combo(Vec::new(), KeyCode::Character('a'), hold)
            .validate()
            .is_err(),
        "a bare letter"
    );
    assert!(
        combo(ctrl.clone(), KeyCode::Function(0), hold)
            .validate()
            .is_err()
    );
    assert!(
        combo(ctrl.clone(), KeyCode::Function(25), hold)
            .validate()
            .is_err()
    );
    assert!(
        combo(ctrl.clone(), KeyCode::Named("  ".into()), hold)
            .validate()
            .is_err()
    );
    assert!(
        combo(ctrl.clone(), KeyCode::Character(' '), hold)
            .validate()
            .is_err(),
        "use Space"
    );
    assert!(
        combo(
            ctrl.clone(),
            KeyCode::Space,
            HotkeyTriggerMode::DoubleTapToggle
        )
        .validate()
        .is_err(),
        "double-tap is for a lone modifier"
    );

    combo(ctrl.clone(), KeyCode::Character('a'), hold)
        .validate()
        .expect("modifier + letter");
    combo(ctrl, KeyCode::Space, hold)
        .validate()
        .expect("modifier + space");
    combo(Vec::new(), KeyCode::Function(13), hold)
        .validate()
        .expect("a function key alone");
    HotkeyBinding::standalone_modifier(
        ShortcutId::new("s"),
        ModifierKey::RightAlt,
        HotkeyTriggerMode::DoubleTapToggle,
    )
    .validate()
    .expect("double-tap on a lone modifier");
}
