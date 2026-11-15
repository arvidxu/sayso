use std::collections::HashSet;

use dictation_types::{AppError, ErrorKind, PermissionKind};

fn every_kind() -> Vec<ErrorKind> {
    vec![
        ErrorKind::Validation,
        ErrorKind::NotFound,
        ErrorKind::AlreadyExists,
        ErrorKind::PermissionDenied(None),
        ErrorKind::PermissionDenied(Some(PermissionKind::Microphone)),
        ErrorKind::AudioDevice,
        ErrorKind::AsrEngine,
        ErrorKind::LlmEngine,
        ErrorKind::TextInjection,
        ErrorKind::Hotkey,
        ErrorKind::Storage,
        ErrorKind::SecretStore,
        ErrorKind::ModelStore,
        ErrorKind::ChecksumMismatch,
        ErrorKind::Network,
        ErrorKind::RateLimited,
        ErrorKind::Timeout,
        ErrorKind::Cancelled,
        ErrorKind::Unsupported,
        ErrorKind::Internal,
        ErrorKind::Custom("extension.quota".to_owned()),
    ]
}

#[test]
fn only_network_timeout_and_rate_limit_failures_are_retryable_by_default() {
    let retryable = [
        ErrorKind::Network,
        ErrorKind::Timeout,
        ErrorKind::RateLimited,
    ];
    for kind in every_kind() {
        let err = AppError::new(kind.clone(), "something happened");
        assert_eq!(err.retryable, retryable.contains(&kind), "{kind:?}");
    }
}

#[test]
fn the_named_constructors_produce_their_own_kind() {
    assert_eq!(AppError::network("x").kind, ErrorKind::Network);
    assert_eq!(AppError::timeout("x").kind, ErrorKind::Timeout);
    assert_eq!(AppError::rate_limited("x").kind, ErrorKind::RateLimited);
    assert_eq!(AppError::validation("x").kind, ErrorKind::Validation);
    assert_eq!(AppError::not_found("x").kind, ErrorKind::NotFound);
    assert_eq!(AppError::already_exists("x").kind, ErrorKind::AlreadyExists);
    assert_eq!(
        AppError::checksum_mismatch("x").kind,
        ErrorKind::ChecksumMismatch
    );
    assert_eq!(AppError::cancelled("x").kind, ErrorKind::Cancelled);
    assert_eq!(
        AppError::permission_denied(Some(PermissionKind::Microphone), "x").kind,
        ErrorKind::PermissionDenied(Some(PermissionKind::Microphone))
    );
    assert!(AppError::cancelled("x").is_cancelled());
    assert!(!AppError::timeout("x").is_cancelled());
}

#[test]
fn the_retry_default_can_be_overridden_either_way() {
    let give_up = AppError::network("certificate rejected").with_retryable(false);
    assert!(!give_up.retryable);
    assert_eq!(give_up.kind, ErrorKind::Network);

    let try_again = AppError::asr("engine was still warming up").with_retryable(true);
    assert!(try_again.retryable);
    assert_eq!(try_again.kind, ErrorKind::AsrEngine);

    for kind in every_kind() {
        assert!(
            AppError::new(kind.clone(), "x")
                .with_retryable(true)
                .retryable
        );
        assert!(!AppError::new(kind, "x").with_retryable(false).retryable);
    }
}

#[test]
fn an_error_category_from_outside_can_be_represented() {
    let err = AppError::new(
        ErrorKind::Custom("translation.glossary-missing".to_owned()),
        "no glossary for sw",
    );
    assert_eq!(
        err.kind,
        ErrorKind::Custom("translation.glossary-missing".to_owned())
    );
    assert_ne!(err.kind, ErrorKind::Custom("translation.other".to_owned()));
    assert_ne!(err.kind, ErrorKind::Internal);
    assert!(!err.retryable);

    let shown = err.to_string();
    assert!(shown.contains("translation.glossary-missing"), "{shown}");
    assert!(shown.contains("no glossary for sw"), "{shown}");
}

#[test]
fn an_error_shows_its_kind_and_message() {
    let shown = AppError::not_found("mode 'x' is not registered").to_string();
    assert!(shown.contains("mode 'x' is not registered"), "{shown}");
    assert_ne!(
        shown, "mode 'x' is not registered",
        "the kind must be shown too"
    );

    let bare = AppError::timeout("").to_string();
    assert!(!bare.is_empty());
    assert!(!bare.ends_with(": "), "{bare:?}");
}

#[test]
fn every_error_kind_has_a_distinct_description() {
    let shown: HashSet<String> = every_kind().iter().map(ToString::to_string).collect();
    assert_eq!(shown.len(), every_kind().len());
}

#[test]
fn a_denied_permission_names_the_permission() {
    let shown = AppError::permission_denied(Some(PermissionKind::Microphone), "x").to_string();
    assert!(shown.contains("microphone"), "{shown}");
}
