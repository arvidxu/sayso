use dictation_types::{LlmCaps, LlmChunk, LlmMessage, LlmRequest, LlmRole, ModelId};

#[test]
fn a_language_model_request_chooses_no_model_or_sampling_for_the_user() {
    let request = LlmRequest::new(vec![
        LlmMessage::system("Réécris le texte."),
        LlmMessage::user("bonjour à tous"),
    ]);
    assert_eq!(request.model_id, None);
    assert_eq!(request.temperature, None);
    assert_eq!(request.max_tokens, None);
    assert_eq!(request.messages.len(), 2);

    let chosen = request.with_model(ModelId::new("model-a"));
    assert_eq!(chosen.model_id, Some(ModelId::new("model-a")));
}

#[test]
fn messages_carry_their_role_and_text_unchanged() {
    assert_eq!(LlmMessage::system("நீ ஒரு உதவியாளர்").role, LlmRole::System);
    assert_eq!(LlmMessage::user("Привет").role, LlmRole::User);
    assert_eq!(LlmMessage::assistant("안녕하세요").role, LlmRole::Assistant);
    assert_eq!(LlmMessage::user("  مرحبا  ").content, "  مرحبا  ");
}

#[test]
fn a_text_chunk_is_not_final_and_reports_no_usage() {
    let chunk = LlmChunk::text("Olá");
    assert_eq!(chunk.delta, "Olá");
    assert!(!chunk.is_final);
    assert_eq!(chunk.usage, None);
}

#[test]
fn a_zero_context_limit_is_invalid() {
    let mut caps = LlmCaps::new(true, true);
    assert!(caps.validate().is_ok());
    assert_eq!(caps.max_context_tokens, None);
    caps.max_context_tokens = Some(0);
    assert!(caps.validate().is_err());
    caps.max_context_tokens = Some(4096);
    assert!(caps.validate().is_ok());
}
