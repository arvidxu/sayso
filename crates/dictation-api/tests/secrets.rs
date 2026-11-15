mod support;

use dictation_api::SecretStore;
use dictation_types::{ErrorKind, SecretString};
use support::MapSecrets;

#[test]
fn a_stored_secret_counts_as_a_secret() {
    let store = MapSecrets::default();
    store
        .set_secret("engine.token", SecretString::new("s3cr3t"))
        .unwrap();
    assert!(store.has_secret("engine.token").unwrap());
}

#[test]
fn a_missing_secret_does_not_count_as_a_secret() {
    let store = MapSecrets::default();
    store
        .set_secret("engine.token", SecretString::new("s3cr3t"))
        .unwrap();
    assert!(!store.has_secret("other.token").unwrap());
}

#[test]
fn an_empty_stored_secret_does_not_count_as_a_secret() {
    let store = MapSecrets::default();
    store
        .set_secret("engine.token", SecretString::new(""))
        .unwrap();
    assert_eq!(
        store.get_secret("engine.token").unwrap(),
        Some(SecretString::new(""))
    );
    assert!(!store.has_secret("engine.token").unwrap());
}

#[test]
fn checking_for_a_secret_fails_when_the_store_fails() {
    let store = MapSecrets {
        broken: true,
        ..MapSecrets::default()
    };
    assert_eq!(
        store.has_secret("engine.token").unwrap_err().kind,
        ErrorKind::SecretStore
    );
}
