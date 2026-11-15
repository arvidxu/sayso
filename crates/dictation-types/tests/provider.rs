mod support;

use dictation_types::{
    AsrCaps, ErrorKind, LlmCaps, ModelId, ProviderCaps, ProviderDescriptor, ProviderId,
    ProviderKind, ProviderRestriction,
};
use support::{mixing_up_to, one_per_utterance};

#[test]
fn a_speech_provider_description_is_of_the_speech_kind() {
    let caps = mixing_up_to(&["ru", "ko", "fr"], None);
    let descriptor =
        ProviderDescriptor::asr(ProviderId::new("speech-a"), "Speech A", caps.clone(), false);
    assert_eq!(descriptor.kind(), ProviderKind::Asr);
    assert_eq!(descriptor.asr_caps(), Some(&caps));
    assert_eq!(descriptor.llm_caps(), None);
    assert_eq!(descriptor.caps, ProviderCaps::Asr(caps));
    assert!(descriptor.validate().is_ok());
}

#[test]
fn a_language_model_provider_description_is_of_the_language_model_kind() {
    let caps = LlmCaps::new(true, true);
    let descriptor =
        ProviderDescriptor::llm(ProviderId::new("writer-a"), "Writer A", caps.clone(), false);
    assert_eq!(descriptor.kind(), ProviderKind::Llm);
    assert_eq!(descriptor.llm_caps(), Some(&caps));
    assert_eq!(descriptor.asr_caps(), None);
    assert!(descriptor.validate().is_ok());
}

#[test]
fn a_new_provider_description_names_no_models_endpoint_or_extension() {
    let descriptor = ProviderDescriptor::asr(
        ProviderId::new("speech-a"),
        "Speech A",
        one_per_utterance(&["sw"]),
        false,
    );
    assert!(descriptor.supported_models.is_empty());
    assert_eq!(descriptor.default_endpoint, None);
    assert_eq!(descriptor.extension_id, None);
}

#[test]
fn an_on_device_provider_needs_no_credential() {
    let speech = ProviderDescriptor::asr(
        ProviderId::new("speech-a"),
        "Speech A",
        one_per_utterance(&["sw"]),
        false,
    );
    let writer = ProviderDescriptor::llm(
        ProviderId::new("writer-a"),
        "Writer A",
        LlmCaps::new(true, true),
        false,
    );
    assert!(!speech.requires_credential);
    assert!(!writer.requires_credential);
}

#[test]
fn a_provider_description_without_an_id_or_a_name_is_invalid() {
    let caps = one_per_utterance(&["sw"]);
    let no_id = ProviderDescriptor::asr(ProviderId::new("   "), "Speech A", caps.clone(), false);
    let no_name = ProviderDescriptor::asr(ProviderId::new("speech-a"), "  ", caps, false);
    assert_eq!(no_id.validate().unwrap_err().kind, ErrorKind::Validation);
    assert_eq!(no_name.validate().unwrap_err().kind, ErrorKind::Validation);
}

#[test]
fn a_provider_description_with_invalid_capabilities_is_invalid() {
    let speech = ProviderDescriptor::asr(
        ProviderId::new("speech-a"),
        "Speech A",
        AsrCaps::new(Vec::new(), 16_000, true),
        false,
    );
    assert!(speech.validate().is_err());

    let mut caps = LlmCaps::new(true, true);
    caps.max_context_tokens = Some(0);
    let writer = ProviderDescriptor::llm(ProviderId::new("writer-a"), "Writer A", caps, false);
    assert!(writer.validate().is_err());
}

#[test]
fn a_provider_description_reports_the_limits_of_its_capabilities() {
    let caps = mixing_up_to(&["ru", "ko", "fr"], Some(2));
    let speech =
        ProviderDescriptor::asr(ProviderId::new("speech-a"), "Speech A", caps.clone(), false);
    assert_eq!(speech.limits(), caps.limits());

    let mut no_stream = LlmCaps::new(true, true);
    no_stream.supports_streaming = false;
    let writer = ProviderDescriptor::llm(ProviderId::new("writer-a"), "Writer A", no_stream, false);
    assert_eq!(writer.limits(), [ProviderRestriction::StreamingUnsupported]);

    let streaming = ProviderDescriptor::llm(
        ProviderId::new("writer-b"),
        "Writer B",
        LlmCaps::new(true, true),
        false,
    );
    assert_eq!(streaming.limits(), []);
}

#[test]
fn provider_description_builders_set_what_they_say() {
    let descriptor = ProviderDescriptor::llm(
        ProviderId::new("writer-a"),
        "Writer A",
        LlmCaps::new(true, true),
        false,
    )
    .with_supported_models(vec![ModelId::new("m-1"), ModelId::new("m-2")])
    .with_requires_credential(true);
    assert_eq!(
        descriptor.supported_models,
        [ModelId::new("m-1"), ModelId::new("m-2")]
    );
    assert!(descriptor.requires_credential);
}
