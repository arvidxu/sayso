use std::collections::HashSet;

use dictation_types::{ErrorKind, OutputTiming};

#[test]
fn every_timing_has_its_own_name() {
    let names: HashSet<&str> = OutputTiming::ALL.iter().map(OutputTiming::as_str).collect();
    assert_eq!(names.len(), OutputTiming::ALL.len());
    for name in names {
        assert!(!name.trim().is_empty());
        assert_eq!(name, name.trim());
    }
}

#[test]
fn the_names_are_the_ones_configuration_uses() {
    assert_eq!(OutputTiming::WhileSpeaking.as_str(), "while-speaking");
    assert_eq!(OutputTiming::AtTheEnd.as_str(), "at-the-end");
}

#[test]
fn a_timing_is_displayed_by_its_name() {
    for timing in OutputTiming::ALL {
        assert_eq!(timing.to_string(), timing.as_str());
    }
}

#[test]
fn a_name_reads_back_as_the_timing_it_came_from() {
    for timing in OutputTiming::ALL {
        assert_eq!(timing.as_str().parse::<OutputTiming>().unwrap(), timing);
        assert_eq!(timing.to_string().parse::<OutputTiming>().unwrap(), timing);
    }
}

#[test]
fn a_name_is_read_whatever_its_letter_case_or_the_space_around_it() {
    for timing in OutputTiming::ALL {
        let name = timing.as_str();
        for written in [
            name.to_uppercase(),
            format!("  {name}"),
            format!("{name}\n"),
        ] {
            assert_eq!(
                written.parse::<OutputTiming>().unwrap(),
                timing,
                "{written:?}"
            );
        }
    }
}

#[test]
fn an_unknown_name_is_refused_and_named_in_the_error() {
    for unknown in [
        "",
        "whenever",
        "while speaking",
        "while_speaking",
        "at-the-end-",
        "end",
    ] {
        let err = unknown.parse::<OutputTiming>().expect_err(unknown);
        assert_eq!(err.kind, ErrorKind::Validation, "{unknown:?}");
        assert!(err.message.contains(unknown), "{unknown:?}: {err}");
    }
}
