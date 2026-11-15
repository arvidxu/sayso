mod support;

use dictation_types::{
    AsrOptions, ErrorKind, LlmCaps, ProviderDetails, ProviderKind, ProviderRestriction,
};
use extension::Registry;
use support::*;

#[test]
fn a_registered_speech_provider_can_be_found_and_used() {
    let mut registry = Registry::new();
    register_speech(
        &mut registry,
        "speech-a",
        "ru+ko+fr",
        "привет 안녕하세요 bonjour",
    )
    .unwrap();

    let id = pid("speech-a");
    let descriptor = registry.providers().get_descriptor(&id).unwrap();
    assert_eq!(descriptor.kind(), ProviderKind::Asr);
    assert_eq!(descriptor.id, id);
    assert_eq!(
        registry.providers().asr_capabilities(&id),
        Some(&speech_caps("ru+ko+fr"))
    );
    assert_eq!(registry.providers().llm_capabilities(&id), None);
    assert!(registry.providers().get_llm(&id).is_none());
    assert_eq!(
        registry.providers().provider_limits(&id),
        Some(speech_caps("ru+ko+fr").limits())
    );
    assert_eq!(
        said_by(&registry, "speech-a").as_deref(),
        Some("привет 안녕하세요 bonjour")
    );
}

#[test]
fn a_registered_language_model_provider_can_be_found_and_used() {
    let mut registry = Registry::new();
    register_writer(&mut registry, "writer-a", "texto limpo").unwrap();

    let id = pid("writer-a");
    let descriptor = registry.providers().get_descriptor(&id).unwrap();
    assert_eq!(descriptor.kind(), ProviderKind::Llm);
    assert_eq!(
        registry.providers().llm_capabilities(&id),
        Some(&LlmCaps::new(true, true))
    );
    assert_eq!(registry.providers().asr_capabilities(&id), None);
    assert!(registry.providers().get_asr(&id).is_none());
    assert_eq!(registry.providers().provider_limits(&id), Some(Vec::new()));
    assert_eq!(
        written_by(&registry, "writer-a").as_deref(),
        Some("texto limpo")
    );
}

#[test]
fn providers_are_listed_by_kind() {
    let mut registry = Registry::new();
    register_speech(&mut registry, "speech-b", "pt", "x").unwrap();
    register_writer(&mut registry, "writer-a", "x").unwrap();
    register_speech(&mut registry, "speech-a", "it", "x").unwrap();

    let mut speech = ids(&registry.providers().list_by_kind(ProviderKind::Asr));
    speech.sort();
    assert_eq!(speech, ["speech-a", "speech-b"]);
    assert_eq!(
        ids(&registry.providers().list_by_kind(ProviderKind::Llm)),
        ["writer-a"]
    );
    assert_eq!(registry.providers().list_descriptors().len(), 3);
}

#[test]
fn a_request_is_checked_against_the_registered_capabilities() {
    let mut registry = Registry::new();
    register_speech(&mut registry, "speech-a", "ru+ko+fr", "x").unwrap();
    let check = |opts: &AsrOptions| {
        registry
            .providers()
            .check_asr_options(&pid("speech-a"), opts, None)
            .unwrap()
    };

    assert!(check(&AsrOptions::new(langs("ko+fr"))).is_empty());
    assert_eq!(
        check(&AsrOptions::new(langs("ko+bn"))),
        [ProviderRestriction::UnsupportedLanguages(
            langs("bn").as_slice().to_vec()
        )]
    );
    let mut per_segment = AsrOptions::new(langs("ru+ko"));
    per_segment.per_segment_language = true;
    assert_eq!(
        check(&per_segment),
        [ProviderRestriction::PerSegmentLanguageUnsupported]
    );
}

#[test]
fn checking_a_request_against_an_unknown_or_non_speech_provider_fails() {
    let mut registry = Registry::new();
    register_writer(&mut registry, "writer-a", "x").unwrap();
    let opts = AsrOptions::new(langs("pt"));

    let err = registry
        .providers()
        .check_asr_options(&pid("missing"), &opts, None)
        .unwrap_err();
    assert_eq!(err.kind, ErrorKind::NotFound);
    assert!(
        registry
            .providers()
            .check_asr_options(&pid("writer-a"), &opts, None)
            .is_err()
    );
}

#[test]
fn a_duplicate_id_is_refused_within_and_across_kinds() {
    let mut registry = Registry::new();
    register_speech(&mut registry, "speech-a", "pt+it", "first").unwrap();
    let err = register_speech(&mut registry, "speech-a", "bn+en", "second").unwrap_err();
    assert_eq!(err.kind, ErrorKind::AlreadyExists);
    let err = register_writer(&mut registry, "speech-a", "writer").unwrap_err();
    assert_eq!(err.kind, ErrorKind::AlreadyExists);

    assert_eq!(said_by(&registry, "speech-a").as_deref(), Some("first"));
    assert_eq!(written_by(&registry, "speech-a"), None);
    assert_eq!(
        registry.providers().asr_capabilities(&pid("speech-a")),
        Some(&speech_caps("pt+it"))
    );

    register_writer(&mut registry, "writer-a", "first").unwrap();
    let err = register_writer(&mut registry, "writer-a", "second").unwrap_err();
    assert_eq!(err.kind, ErrorKind::AlreadyExists);
    assert!(register_speech(&mut registry, "writer-a", "pt+it", "speech").is_err());
    assert_eq!(written_by(&registry, "writer-a").as_deref(), Some("first"));
    assert_eq!(said_by(&registry, "writer-a"), None);
}

#[test]
fn an_explicit_replacement_takes_over_the_id_or_registers_when_it_is_free() {
    let mut registry = Registry::new();
    register_speech(&mut registry, "speech-a", "pt+it", "first").unwrap();
    upsert_speech(&mut registry, "speech-a", "bn+en", "second").unwrap();

    assert_eq!(said_by(&registry, "speech-a").as_deref(), Some("second"));
    assert_eq!(
        registry.providers().asr_capabilities(&pid("speech-a")),
        Some(&speech_caps("bn+en"))
    );
    assert_eq!(registry.providers().list_descriptors().len(), 1);

    register_writer(&mut registry, "writer-a", "first").unwrap();
    upsert_writer(&mut registry, "writer-a", "second").unwrap();
    assert_eq!(written_by(&registry, "writer-a").as_deref(), Some("second"));
    assert_eq!(registry.providers().list_descriptors().len(), 2);

    upsert_speech(&mut registry, "free-speech", "pt+it", "x").unwrap();
    upsert_writer(&mut registry, "free-writer", "y").unwrap();
    assert_eq!(said_by(&registry, "free-speech").as_deref(), Some("x"));
    assert_eq!(written_by(&registry, "free-writer").as_deref(), Some("y"));
}

#[test]
fn a_refused_replacement_leaves_the_existing_provider_untouched() {
    let mut registry = Registry::new();
    register_speech(&mut registry, "speech-a", "pt+it", "first").unwrap();

    let engine = speech_engine("speech-a", "bn+en", "second");
    let unnamed = ProviderDetails::new("   ", false);
    assert!(registry.upsert_asr_provider(unnamed, engine).is_err());

    assert_eq!(said_by(&registry, "speech-a").as_deref(), Some("first"));
    assert_eq!(
        registry.providers().asr_capabilities(&pid("speech-a")),
        Some(&speech_caps("pt+it"))
    );
}

#[test]
fn replacing_a_speech_provider_with_a_language_model_leaves_no_trace_of_the_speech_engine() {
    let mut registry = Registry::new();
    register_speech(&mut registry, "shape-shifter", "pt+it", "speech").unwrap();
    upsert_writer(&mut registry, "shape-shifter", "writer").unwrap();

    let id = pid("shape-shifter");
    let providers = registry.providers();
    assert!(
        providers.get_asr(&id).is_none(),
        "the old speech engine is still reachable"
    );
    assert_eq!(said_by(&registry, "shape-shifter"), None);
    assert_eq!(providers.asr_capabilities(&id), None);
    assert_eq!(
        written_by(&registry, "shape-shifter").as_deref(),
        Some("writer")
    );
    assert_eq!(
        providers.get_descriptor(&id).unwrap().kind(),
        ProviderKind::Llm
    );
    assert!(providers.list_by_kind(ProviderKind::Asr).is_empty());
    assert_eq!(
        ids(&providers.list_by_kind(ProviderKind::Llm)),
        ["shape-shifter"]
    );
    assert_eq!(providers.list_descriptors().len(), 1);
    assert!(
        providers
            .check_asr_options(&id, &AsrOptions::new(langs("pt")), None)
            .is_err(),
        "it is no longer a speech provider"
    );
}

#[test]
fn replacing_a_language_model_with_a_speech_provider_leaves_no_trace_of_the_language_model() {
    let mut registry = Registry::new();
    register_writer(&mut registry, "shape-shifter", "writer").unwrap();
    upsert_speech(&mut registry, "shape-shifter", "ko+ja", "speech").unwrap();

    let id = pid("shape-shifter");
    let providers = registry.providers();
    assert!(
        providers.get_llm(&id).is_none(),
        "the old engine is still reachable"
    );
    assert_eq!(written_by(&registry, "shape-shifter"), None);
    assert_eq!(providers.llm_capabilities(&id), None);
    assert_eq!(
        said_by(&registry, "shape-shifter").as_deref(),
        Some("speech")
    );
    assert_eq!(
        providers.get_descriptor(&id).unwrap().kind(),
        ProviderKind::Asr
    );
    assert!(providers.list_by_kind(ProviderKind::Llm).is_empty());
    assert_eq!(providers.list_descriptors().len(), 1);
}

#[test]
fn removing_a_provider_removes_its_description_and_its_engine() {
    let mut registry = Registry::new();
    register_speech(&mut registry, "speech-a", "pt+it", "x").unwrap();
    register_writer(&mut registry, "writer-a", "y").unwrap();

    assert!(registry.unregister_provider(&pid("speech-a")));
    assert!(
        !registry.unregister_provider(&pid("speech-a")),
        "already removed"
    );
    assert!(!registry.unregister_provider(&pid("never-registered")));
    let providers = registry.providers();
    assert!(providers.get_descriptor(&pid("speech-a")).is_none());
    assert!(providers.get_asr(&pid("speech-a")).is_none());
    assert_eq!(written_by(&registry, "writer-a").as_deref(), Some("y"));

    assert!(registry.unregister_provider(&pid("writer-a")));
    assert!(registry.providers().get_llm(&pid("writer-a")).is_none());
    assert!(registry.providers().list_descriptors().is_empty());

    register_speech(&mut registry, "speech-a", "bn+en", "again").unwrap();
    assert_eq!(said_by(&registry, "speech-a").as_deref(), Some("again"));
}
