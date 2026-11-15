mod support;

use dictation_api::LlmEngine;
use dictation_types::{AppError, ErrorKind, LlmChunk, TokenUsage};
use support::{ChunkEngine, request};

fn last_chunk(delta: &str, usage: Option<TokenUsage>) -> LlmChunk {
    LlmChunk {
        delta: delta.to_owned(),
        is_final: true,
        usage,
    }
}

#[test]
fn collected_completion_joins_the_chunks_in_order() {
    let engine = ChunkEngine::new(vec![
        Ok(LlmChunk::text("Bom dia, ")),
        Ok(LlmChunk::text("こんにちは、")),
        Ok(LlmChunk::text("")),
        Ok(LlmChunk::text("Привет ")),
        Ok(last_chunk("안녕하세요.", None)),
    ]);
    let completion = engine.complete_text(request()).unwrap();
    assert_eq!(completion.text, "Bom dia, こんにちは、Привет 안녕하세요.");
    assert_eq!(completion.usage, None);
}

#[test]
fn collected_completion_adds_nothing_between_chunks() {
    let engine = ChunkEngine::new(vec![
        Ok(LlmChunk::text("স্বা")),
        Ok(LlmChunk::text("গত")),
        Ok(LlmChunk::text("ম")),
    ]);
    assert_eq!(engine.complete_text(request()).unwrap().text, "স্বাগতম");
}

#[test]
fn collected_completion_carries_the_reported_token_usage() {
    let usage = TokenUsage::new(42, 7);
    let engine = ChunkEngine::new(vec![
        Ok(LlmChunk::text("Olá")),
        Ok(last_chunk(" mundo", Some(usage))),
    ]);
    let completion = engine.complete_text(request()).unwrap();
    assert_eq!(completion.text, "Olá mundo");
    assert_eq!(completion.usage, Some(usage));
}

#[test]
fn collected_completion_keeps_usage_reported_before_the_last_chunk() {
    let usage = TokenUsage::new(10, 3);
    let mut early = LlmChunk::text("Ciao");
    early.usage = Some(usage);
    let engine = ChunkEngine::new(vec![Ok(early), Ok(LlmChunk::text(" a tutti"))]);
    assert_eq!(engine.complete_text(request()).unwrap().usage, Some(usage));
}

#[test]
fn collected_completion_reports_the_usage_reported_last() {
    let mut first = LlmChunk::text("a");
    first.usage = Some(TokenUsage::new(10, 1));
    let engine = ChunkEngine::new(vec![
        Ok(first),
        Ok(last_chunk("b", Some(TokenUsage::new(10, 2)))),
    ]);
    assert_eq!(
        engine.complete_text(request()).unwrap().usage,
        Some(TokenUsage::new(10, 2))
    );
}

#[test]
fn collected_completion_of_an_empty_stream_is_empty() {
    let completion = ChunkEngine::new(Vec::new())
        .complete_text(request())
        .unwrap();
    assert_eq!(completion.text, "");
    assert_eq!(completion.usage, None);
}

#[test]
fn a_failure_mid_stream_fails_the_whole_collected_completion() {
    let engine = ChunkEngine::new(vec![
        Ok(LlmChunk::text("this part arrived")),
        Err(AppError::network("connection dropped")),
        Ok(last_chunk(
            " and this came after",
            Some(TokenUsage::new(1, 1)),
        )),
    ]);
    let err = engine.complete_text(request()).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Network);
    assert_eq!(err.message, "connection dropped");
}

#[test]
fn a_failure_on_the_last_item_fails_the_whole_collected_completion() {
    let engine = ChunkEngine::new(vec![
        Ok(LlmChunk::text("almost")),
        Ok(LlmChunk::text(" done")),
        Err(AppError::llm("ran out of memory")),
    ]);
    assert_eq!(
        engine.complete_text(request()).unwrap_err().kind,
        ErrorKind::LlmEngine
    );
}

#[test]
fn a_request_that_cannot_start_fails_the_collected_completion() {
    let mut engine = ChunkEngine::new(vec![Ok(LlmChunk::text("never sent"))]);
    engine.refuse = Some(AppError::rate_limited("busy"));
    assert_eq!(
        engine.complete_text(request()).unwrap_err().kind,
        ErrorKind::RateLimited
    );
}
