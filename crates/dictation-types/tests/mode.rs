use std::collections::HashSet;

use dictation_types::{ContextQuery, ErrorKind, LlmStep, Mode, ModeId, PromptVariable};

fn llm_mode(prompt: &str) -> Mode {
    Mode::with_llm(ModeId::new("mode-x"), "Mode X", prompt)
}

fn plain_mode() -> Mode {
    Mode::without_llm(ModeId::new("mode-plain"), "Plain")
}

#[test]
fn a_mode_without_a_language_model_is_valid_without_a_prompt() {
    let mode = plain_mode();
    assert!(mode.validate().is_ok());
    assert_eq!(mode.llm, None);
}

#[test]
fn a_mode_with_a_language_model_and_a_prompt_is_valid() {
    assert!(
        llm_mode("Corrige la ponctuation de : {text}")
            .validate()
            .is_ok()
    );
    assert!(
        llm_mode("次の文章を整えてください：{text}")
            .validate()
            .is_ok()
    );
}

#[test]
fn a_mode_that_uses_a_language_model_must_have_a_prompt() {
    for prompt in ["", " ", "\n\t  "] {
        let err = llm_mode(prompt).validate().unwrap_err();
        assert_eq!(err.kind, ErrorKind::Validation, "{prompt:?}");
    }
    let mut built_by_hand = plain_mode();
    built_by_hand.llm = Some(LlmStep {
        prompt_template: String::new(),
        skip_under_chars: Some(10),
    });
    assert!(built_by_hand.validate().is_err());
}

#[test]
fn a_mode_needs_an_id_and_a_name() {
    assert!(
        Mode::without_llm(ModeId::new(""), "Name")
            .validate()
            .is_err()
    );
    assert!(
        Mode::without_llm(ModeId::new("   "), "Name")
            .validate()
            .is_err()
    );
    assert!(
        Mode::without_llm(ModeId::new("mode-x"), "")
            .validate()
            .is_err()
    );
    assert!(
        Mode::without_llm(ModeId::new("mode-x"), "  ")
            .validate()
            .is_err()
    );
    assert!(
        Mode::with_llm(ModeId::new(""), "Name", "{text}")
            .validate()
            .is_err()
    );
}

#[test]
fn a_new_mode_decides_no_target_language() {
    assert_eq!(plain_mode().target_language, None);
    assert_eq!(llm_mode("{text}").target_language, None);
}

#[test]
fn a_mode_name_can_be_in_any_language() {
    for name in ["Rédaction", "Переписать", "다듬기", "整理", "পরিষ্কার"]
    {
        assert!(
            Mode::without_llm(ModeId::new("m"), name).validate().is_ok(),
            "{name}"
        );
    }
}

#[test]
fn placeholders_are_discovered_from_the_prompt_text() {
    let mode = llm_mode("Rewrite {text} for a {scene} app. Reply in {target_language}.");
    assert_eq!(mode.placeholders(), ["text", "scene", "target_language"]);
}

#[test]
fn placeholders_are_listed_once_in_order_of_first_appearance() {
    let mode = llm_mode("{terms} then {text}, again {terms}, and {languages} with {text}");
    assert_eq!(mode.placeholders(), ["terms", "text", "languages"]);
}

#[test]
fn placeholders_the_dictation_flow_does_not_know_are_discovered_too() {
    let mode = llm_mode("Tone: {tone_2}. Audience: {audience}. Input: {text}");
    assert_eq!(mode.placeholders(), ["tone_2", "audience", "text"]);
    assert!(mode.uses(PromptVariable::Text));
    assert!(!mode.uses(PromptVariable::SelectedText));
}

#[test]
fn a_prompt_without_placeholders_has_none() {
    assert!(
        llm_mode("Corrige o texto seguinte.")
            .placeholders()
            .is_empty()
    );
    assert!(plain_mode().placeholders().is_empty());
}

#[test]
fn braces_that_do_not_hold_a_name_are_not_placeholders() {
    let mode = llm_mode(r#"Answer as JSON like {"result": "..."} or {} or { } using {clipboard}"#);
    assert_eq!(mode.placeholders(), ["clipboard"]);

    let unclosed = llm_mode("Start {text} and then an open brace { with no end");
    assert_eq!(unclosed.placeholders(), ["text"]);
}

#[test]
fn a_placeholder_inside_other_braces_is_still_found() {
    let mode = llm_mode("Return {\"selection\": \"{selected}\", \"input\": \"{text}\"}");
    assert_eq!(mode.placeholders(), ["selected", "text"]);
}

#[test]
fn every_flow_variable_is_recognised_by_its_placeholder() {
    for variable in PromptVariable::ALL {
        let mode = llm_mode(&format!("Before {} after", variable.placeholder()));
        assert!(mode.uses(variable), "{variable:?}");
        for other in PromptVariable::ALL {
            if other != variable {
                assert!(
                    !mode.uses(other),
                    "{variable:?} prompt must not use {other:?}"
                );
            }
        }
    }
}

#[test]
fn flow_variables_have_distinct_names_written_in_braces() {
    let names: HashSet<&str> = PromptVariable::ALL
        .iter()
        .map(PromptVariable::name)
        .collect();
    assert_eq!(names.len(), PromptVariable::ALL.len());
    for variable in PromptVariable::ALL {
        assert_eq!(variable.placeholder(), format!("{{{}}}", variable.name()));
    }
}

#[test]
fn a_word_in_the_prompt_is_not_a_placeholder_without_braces() {
    let mode = llm_mode("Mention the selected text and the clipboard, then fix {text}.");
    assert!(!mode.uses(PromptVariable::SelectedText));
    assert!(!mode.uses(PromptVariable::ClipboardText));
    assert!(mode.uses(PromptVariable::Text));
}

#[test]
fn a_mode_without_a_language_model_uses_no_variables() {
    for variable in PromptVariable::ALL {
        assert!(!plain_mode().uses(variable));
    }
}

#[test]
fn the_language_model_runs_for_any_length_when_no_threshold_is_set() {
    let mode = llm_mode("{text}");
    for count in [0, 1, 10, 10_000] {
        assert!(mode.should_run_llm(count));
    }
}

#[test]
fn the_language_model_is_skipped_for_input_shorter_than_the_threshold() {
    let mut mode = llm_mode("{text}");
    mode.llm.as_mut().unwrap().skip_under_chars = Some(12);
    assert!(!mode.should_run_llm(0));
    assert!(!mode.should_run_llm(11));
}

#[test]
fn the_language_model_runs_for_input_exactly_at_the_threshold() {
    let mut mode = llm_mode("{text}");
    mode.llm.as_mut().unwrap().skip_under_chars = Some(12);
    assert!(mode.should_run_llm(12));
    assert!(mode.should_run_llm(13));
}

#[test]
fn a_mode_without_a_language_model_never_runs_one() {
    for count in [0, 1, 1_000_000] {
        assert!(!plain_mode().should_run_llm(count));
    }
}

#[test]
fn a_mode_whose_prompt_asks_for_nothing_extra_reads_only_the_active_app() {
    for mode in [
        plain_mode(),
        llm_mode("Fix {text} for {scene} in {languages} with {terms}"),
    ] {
        let query = ContextQuery::for_mode(&mode);
        assert!(query.include_active_app);
        assert!(
            !query.include_selected_text,
            "the selection must not be read"
        );
        assert!(!query.include_clipboard, "the clipboard must not be read");
        assert_eq!(query, ContextQuery::minimal());
    }
}

#[test]
fn the_selection_is_read_only_when_the_prompt_asks_for_it() {
    let query = ContextQuery::for_mode(&llm_mode("Apply {text} to: {selected}"));
    assert!(query.include_selected_text);
    assert!(!query.include_clipboard);
    assert!(query.include_active_app);
}

#[test]
fn the_clipboard_is_read_only_when_the_prompt_asks_for_it() {
    let query = ContextQuery::for_mode(&llm_mode("Use {clipboard} as reference for {text}"));
    assert!(query.include_clipboard);
    assert!(!query.include_selected_text);
    assert!(query.include_active_app);
}

#[test]
fn both_are_read_when_the_prompt_asks_for_both() {
    let query = ContextQuery::for_mode(&llm_mode("{selected} {clipboard} {text}"));
    assert_eq!(query, ContextQuery::all());
}

#[test]
fn the_default_query_reads_nothing() {
    let query = ContextQuery::default();
    assert!(!query.include_active_app);
    assert!(!query.include_selected_text);
    assert!(!query.include_clipboard);
}
