mod support;

use std::fs;

use dictation_api::ModelStore;
use dictation_types::ErrorKind;
use model_store::LocalModelStore;
use support::{TempDir, catalog, content, file_entry, id, model_entry, store_with};

/// A store with the models "a", "b" and "c", where "a" and "b" are installed.
fn store_in(root: &TempDir) -> LocalModelStore {
    let urls = ["https://example.invalid/unused".to_owned()];
    let store = store_with(
        catalog(
            ["a", "b", "c"]
                .iter()
                .enumerate()
                .map(|(i, name)| {
                    model_entry(
                        name,
                        vec![file_entry("m.bin", &content(300, i as u8), &urls)],
                    )
                })
                .collect(),
        ),
        root.path(),
        Vec::new(),
    );
    for (i, name) in ["a", "b"].iter().enumerate() {
        let source = TempDir::new("loaded-source");
        fs::write(source.join("m.bin"), content(300, i as u8)).unwrap();
        store.import_local(&id(name), source.path()).unwrap();
    }
    store
}

fn is_loaded(store: &LocalModelStore, name: &str) -> bool {
    store
        .get_status(&id(name))
        .unwrap()
        .unwrap()
        .state
        .is_loaded()
}

#[test]
fn a_model_that_is_not_installed_cannot_be_marked_loaded() {
    let root = TempDir::new("loaded-refused");
    let store = store_in(&root);
    assert!(store.mark_loaded(&id("c"), true).is_err());
    assert!(!is_loaded(&store, "c"));
}

#[test]
fn marking_a_model_loaded_and_unloaded_shows_in_its_status() {
    let root = TempDir::new("loaded-status");
    let store = store_in(&root);
    assert!(!is_loaded(&store, "a"));
    store.mark_loaded(&id("a"), true).unwrap();
    assert!(is_loaded(&store, "a"));
    assert!(!is_loaded(&store, "b"));
    store.mark_loaded(&id("a"), false).unwrap();
    assert!(!is_loaded(&store, "a"));
}

#[test]
fn marking_a_model_loaded_or_unloaded_twice_changes_nothing() {
    let root = TempDir::new("loaded-twice");
    let store = store_in(&root);
    store.mark_loaded(&id("a"), true).unwrap();
    store.mark_loaded(&id("a"), true).unwrap();
    assert!(is_loaded(&store, "a"));
    store.mark_loaded(&id("a"), false).unwrap();
    store.mark_loaded(&id("a"), false).unwrap();
    assert!(!is_loaded(&store, "a"));
    assert!(
        store
            .get_status(&id("a"))
            .unwrap()
            .unwrap()
            .state
            .is_installed(),
        "unloading does not uninstall"
    );
}

#[test]
fn a_loaded_model_stays_loaded_until_the_store_is_told_otherwise() {
    let root = TempDir::new("loaded-stays");
    let store = store_in(&root);
    store.mark_loaded(&id("a"), true).unwrap();
    store.mark_loaded(&id("b"), true).unwrap();
    store.mark_loaded(&id("b"), false).unwrap();
    for _ in 0..3 {
        assert!(is_loaded(&store, "a"));
        assert!(!is_loaded(&store, "b"));
        assert_eq!(store.list_statuses().unwrap().len(), 3);
    }
}

#[test]
fn a_loaded_model_cannot_be_deleted_until_it_is_marked_unloaded() {
    let root = TempDir::new("loaded-delete");
    let store = store_in(&root);
    store.mark_loaded(&id("a"), true).unwrap();
    let err = store.delete(&id("a")).unwrap_err();
    assert_eq!(err.kind, ErrorKind::AlreadyExists, "{err}");
    assert!(is_loaded(&store, "a"));
    assert!(store.local_path(&id("a")).unwrap().is_some());

    store.mark_loaded(&id("a"), false).unwrap();
    assert!(store.delete(&id("a")).unwrap());
    assert!(store.local_path(&id("a")).unwrap().is_none());
}

#[test]
fn a_loaded_model_is_not_replaced_by_an_import() {
    let root = TempDir::new("loaded-import");
    let store = store_in(&root);
    store.mark_loaded(&id("a"), true).unwrap();
    let source = TempDir::new("loaded-import-source");
    fs::write(source.join("m.bin"), content(300, 0)).unwrap();
    let err = store.import_local(&id("a"), source.path()).unwrap_err();
    assert_eq!(err.kind, ErrorKind::AlreadyExists, "{err}");
    assert!(is_loaded(&store, "a"));
}
