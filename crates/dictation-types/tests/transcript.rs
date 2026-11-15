mod support;

use dictation_types::{
    ModelId, ProviderId, RequestRoute, StageTimings, TokenUsage, Transcript, TranscriptId,
    TranscriptSegment,
};
use support::{from_segments, langs, seg, transcript};

#[test]
fn a_transcript_built_from_segments_has_their_joined_text() {
    let built = from_segments(vec![seg("hello"), seg("world")], "en");
    assert_eq!(built.raw_text, "hello world");
    assert_eq!(built.effective_text(), "hello world");
    assert_eq!(built.segments.len(), 2);

    let chinese = from_segments(vec![seg("今天"), seg("天气"), seg("很好")], "zh+en");
    assert_eq!(chinese.raw_text, "今天天气很好");

    let russian_korean = from_segments(vec![seg("привет"), seg("мир"), seg("안녕하세요")], "ru+ko");
    assert_eq!(russian_korean.raw_text, "привет мир 안녕하세요");
}

#[test]
fn a_transcript_built_from_segments_lasts_until_the_latest_segment_end() {
    let segments = vec![
        TranscriptSegment::new("bom", 0, 400).unwrap(),
        TranscriptSegment::new("dia", 400, 2_750).unwrap(),
        TranscriptSegment::new("a todos", 900, 1_800).unwrap(),
    ];
    assert_eq!(from_segments(segments, "pt").audio_duration_ms, 2_750);
    assert_eq!(from_segments(Vec::new(), "pt").audio_duration_ms, 0);
}

#[test]
fn a_transcript_is_partial_while_any_segment_is_not_final() {
    let done = from_segments(vec![seg("bom"), seg("dia")], "pt");
    assert!(!done.is_partial);

    let pending = from_segments(vec![seg("bom"), seg("dia").with_final(false)], "pt");
    assert!(pending.is_partial);
}

#[test]
fn a_transcript_keeps_the_languages_the_caller_supplied() {
    assert_eq!(transcript("olá", "pt-BR+it").languages, langs("pt-BR+it"));
    assert_eq!(
        from_segments(vec![seg("x")], "ru+ko+fr").languages,
        langs("ru+ko+fr")
    );
    let partial = Transcript::partial(TranscriptId::new("t-2"), "ol", langs("pt+it"));
    assert!(partial.is_partial);
    assert_eq!(partial.languages, langs("pt+it"));
}

#[test]
fn a_segment_cannot_end_before_it_starts() {
    assert!(TranscriptSegment::new("x", 500, 499).is_err());
    assert!(TranscriptSegment::new("x", 500, 500).is_ok());
    assert_eq!(
        TranscriptSegment::new("x", 250, 1_000)
            .unwrap()
            .duration_ms(),
        750
    );
}

#[test]
fn a_new_segment_is_final_and_reports_no_language_or_confidence() {
    let segment = seg("नमस्ते");
    assert!(segment.is_final);
    assert_eq!(segment.language, None);
    assert_eq!(segment.confidence, None);
}

#[test]
fn segment_confidence_stays_between_zero_and_one() {
    assert_eq!(seg("x").with_confidence(1.7).confidence, Some(1.0));
    assert_eq!(seg("x").with_confidence(-0.2).confidence, Some(0.0));
    assert_eq!(seg("x").with_confidence(0.25).confidence, Some(0.25));
}

#[test]
fn before_processing_the_text_to_use_is_the_raw_text() {
    let fresh = transcript("um, olá a todos", "pt");
    assert_eq!(fresh.final_text, None);
    assert_eq!(fresh.effective_text(), "um, olá a todos");
}

#[test]
fn after_processing_the_text_to_use_is_the_processed_text() {
    let cleaned = transcript("um, olá a todos", "pt").with_final_text("Olá a todos.");
    assert_eq!(cleaned.effective_text(), "Olá a todos.");
    assert_eq!(cleaned.raw_text, "um, olá a todos");
}

#[test]
fn an_empty_result_is_not_replaced_by_the_raw_text() {
    for (raw, spec) in [
        ("um uh er", "en"),
        ("hã, tipo, né", "pt+it"),
        ("ну, э-э, вот", "ru+ko+fr"),
        ("えーと、あのー", "ja+en"),
        ("嗯，那个", "zh+en"),
    ] {
        let emptied = transcript(raw, spec).with_final_text("");
        assert_eq!(emptied.final_text.as_deref(), Some(""));
        assert_eq!(
            emptied.effective_text(),
            "",
            "raw text '{raw}' must not come back"
        );
        assert_eq!(emptied.raw_text, raw);
    }
}

#[test]
fn an_empty_result_stays_empty_for_a_transcript_built_from_segments() {
    let emptied = from_segments(vec![seg("euh"), seg("ben")], "fr+en").with_final_text("");
    assert_eq!(emptied.effective_text(), "");
}

#[test]
fn a_processed_result_of_only_whitespace_is_kept_as_given() {
    let spaced = transcript("uh", "en").with_final_text(" ");
    assert_eq!(spaced.effective_text(), " ");
}

#[test]
fn any_named_stage_can_be_timed() {
    let mut timings = StageTimings::default();
    for (stage, ms) in [
        ("capture", 120),
        ("recognition", 480),
        ("a-stage-nobody-has-heard-of", 7),
        ("extension.translate", 33),
        ("étape personnalisée", 5),
        ("후처리", 9),
    ] {
        timings.record(stage, ms);
        assert_eq!(timings.get(stage), Some(ms), "{stage}");
    }
    assert_eq!(timings.stages.len(), 6);
    assert_eq!(timings.get("never recorded"), None);
}

#[test]
fn stages_are_kept_in_the_order_they_were_first_recorded() {
    let mut timings = StageTimings::default();
    timings.record("zeta", 1);
    timings.record("alpha", 2);
    timings.record("mid", 3);
    timings.record("alpha", 4);
    let names: Vec<&str> = timings.stages.iter().map(|t| t.stage.as_str()).collect();
    assert_eq!(names, ["zeta", "alpha", "mid"]);
}

#[test]
fn recording_the_same_stage_twice_accumulates() {
    let mut timings = StageTimings::default();
    timings.record("rewrite", 100);
    timings.record("rewrite", 250);
    timings.record("rewrite", 1);
    assert_eq!(timings.get("rewrite"), Some(351));
    assert_eq!(timings.stages.len(), 1);
}

#[test]
fn accumulating_a_stage_does_not_overflow() {
    let mut timings = StageTimings::default();
    timings.record("long", u64::MAX);
    timings.record("long", 10);
    assert_eq!(timings.get("long"), Some(u64::MAX));
}

#[test]
fn the_total_is_the_sum_of_the_stages_when_none_was_measured() {
    let mut timings = StageTimings::default();
    assert_eq!(timings.computed_total_ms(), 0);
    timings.record("a", 100);
    timings.record("b", 250);
    timings.record("a", 50);
    assert_eq!(timings.total_ms, None);
    assert_eq!(timings.computed_total_ms(), 400);
}

#[test]
fn the_total_is_the_measured_total_when_present() {
    let mut timings = StageTimings::default();
    timings.record("a", 100);
    timings.record("b", 250);
    timings.total_ms = Some(900);
    assert_eq!(timings.computed_total_ms(), 900);

    timings.total_ms = Some(0);
    assert_eq!(timings.computed_total_ms(), 0);
}

#[test]
fn a_transcript_carries_the_timings_it_was_given() {
    let mut timings = StageTimings::default();
    timings.record("recognition", 300);
    let timed = transcript("olá", "pt").with_timings(timings.clone());
    assert_eq!(timed.timings, timings);
    assert_eq!(transcript("olá", "pt").timings, StageTimings::default());
}

#[test]
fn a_local_route_records_that_the_request_stayed_on_the_device() {
    let route = RequestRoute::local(
        "recognition",
        ProviderId::new("speech-a"),
        Some(ModelId::new("m-1")),
    );
    assert!(route.is_local());
    assert_eq!(route.endpoint, None);
    assert_eq!(route.stage, "recognition");
    assert_eq!(route.provider_id, ProviderId::new("speech-a"));
    assert_eq!(route.model_id, Some(ModelId::new("m-1")));
}

#[test]
fn a_remote_route_records_that_the_request_left_the_device_and_where_to() {
    let route = RequestRoute::remote(
        "rewrite",
        ProviderId::new("writer-a"),
        None,
        "https://example.invalid/v1",
    );
    assert!(!route.is_local());
    assert_eq!(
        route.endpoint.as_deref(),
        Some("https://example.invalid/v1")
    );
    assert_eq!(route.model_id, None);
}

#[test]
fn a_transcript_keeps_its_routes_in_order() {
    let mut routed = transcript("olá", "pt");
    assert!(routed.request_routes.is_empty());
    routed.add_route(RequestRoute::local(
        "recognition",
        ProviderId::new("speech-a"),
        None,
    ));
    routed.add_route(RequestRoute::local(
        "rewrite",
        ProviderId::new("writer-a"),
        None,
    ));
    let stages: Vec<&str> = routed
        .request_routes
        .iter()
        .map(|r| r.stage.as_str())
        .collect();
    assert_eq!(stages, ["recognition", "rewrite"]);
    assert!(routed.request_routes.iter().all(RequestRoute::is_local));
}

#[test]
fn token_usage_totals_prompt_and_completion() {
    let usage = TokenUsage::new(120, 30);
    assert_eq!(usage.total_tokens, 150);
    assert_eq!(TokenUsage::new(u32::MAX, 5).total_tokens, u32::MAX);
    assert_eq!(transcript("olá", "pt").token_usage, None);
}
