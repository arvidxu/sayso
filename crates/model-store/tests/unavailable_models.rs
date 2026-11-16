mod support;

use std::fs;

use dictation_api::ModelStore;
use dictation_types::{ErrorKind, ModelInstallState, PlatformTarget};
use model_store::{LocalModelStore, current_platform};
use serde_json::json;
use support::server::{Reply, TestServer};
use support::{
    TempDir, catalog, content, file_entry, id, ignore, model_entry, never, store,
    system_model_entry,
};

fn mixed_store(server: &TestServer, root: &TempDir) -> LocalModelStore {
    let bytes = content(500, 81);
    server.route("/known.bin", Reply::file(&bytes));
    store(
        catalog(vec![
            model_entry(
                "known",
                vec![file_entry("known.bin", &bytes, &[server.url("/known.bin")])],
            ),
            system_model_entry("built-in"),
        ]),
        root.path(),
    )
}

#[test]
fn a_system_provided_model_cannot_be_installed_or_imported() {
    let server = TestServer::start();
    let root = TempDir::new("system-model");
    let store = mixed_store(&server, &root);
    let source = TempDir::new("system-model-source");
    fs::write(source.join("anything.bin"), b"data").unwrap();

    let err = store
        .install(&id("built-in"), &ignore, &never())
        .unwrap_err();
    assert_eq!(err.kind, ErrorKind::Unsupported, "{err}");
    let err = store
        .import_local(&id("built-in"), source.path())
        .unwrap_err();
    assert_eq!(err.kind, ErrorKind::Unsupported, "{err}");
    assert!(server.requests().is_empty());
}

#[test]
fn every_operation_on_an_id_not_in_the_catalog_fails_as_not_found() {
    let server = TestServer::start();
    let root = TempDir::new("unknown-model");
    let store = mixed_store(&server, &root);
    let source = TempDir::new("unknown-model-source");
    let unknown = id("not-in-the-catalog");

    assert!(store.get_status(&unknown).unwrap().is_none());
    let errors = [
        (
            "install",
            store.install(&unknown, &ignore, &never()).map(|_| ()),
        ),
        (
            "import_local",
            store.import_local(&unknown, source.path()).map(|_| ()),
        ),
        ("local_path", store.local_path(&unknown).map(|_| ())),
        ("delete", store.delete(&unknown).map(|_| ())),
        ("mark_loaded true", store.mark_loaded(&unknown, true)),
        ("mark_loaded false", store.mark_loaded(&unknown, false)),
    ];
    for (operation, result) in errors {
        let err = result.expect_err(operation);
        assert_eq!(err.kind, ErrorKind::NotFound, "{operation}: {err}");
    }
    assert!(server.requests().is_empty());
}

#[test]
fn an_id_differing_only_in_letter_case_is_not_the_same_model() {
    let server = TestServer::start();
    let root = TempDir::new("case-model");
    let store = mixed_store(&server, &root);
    assert!(store.get_status(&id("KNOWN")).unwrap().is_none());
    let err = store.install(&id("KNOWN"), &ignore, &never()).unwrap_err();
    assert_eq!(err.kind, ErrorKind::NotFound, "{err}");
}

#[test]
fn the_catalog_lists_every_model_including_system_provided_ones() {
    let server = TestServer::start();
    let root = TempDir::new("list-models");
    let store = mixed_store(&server, &root);
    let ids: Vec<String> = store
        .list_catalog()
        .unwrap()
        .into_iter()
        .map(|entry| entry.id.to_string())
        .collect();
    assert_eq!(ids, ["known", "built-in"]);
    assert_eq!(store.list_statuses().unwrap().len(), 2);
}

#[test]
fn a_system_provided_model_counts_as_installed_without_a_folder_and_can_be_loaded() {
    let server = TestServer::start();
    let root = TempDir::new("system-installed");
    let store = mixed_store(&server, &root);

    let state = store.get_status(&id("built-in")).unwrap().unwrap().state;
    assert_eq!(
        state,
        ModelInstallState::Installed {
            local_path: String::new(),
            loaded_in_memory: false
        }
    );
    assert_eq!(store.local_path(&id("built-in")).unwrap(), None);
    store
        .mark_loaded(&id("built-in"), true)
        .expect("a system model can be loaded");
    assert!(
        store
            .get_status(&id("built-in"))
            .unwrap()
            .unwrap()
            .state
            .is_loaded()
    );
}

#[test]
fn a_model_for_other_platforms_cannot_be_installed_here() {
    let Some(here) = current_platform() else {
        return;
    };
    let server = TestServer::start();
    let bytes = content(500, 82);
    server.route("/elsewhere.bin", Reply::file(&bytes));
    let others: Vec<&str> = PlatformTarget::ALL
        .iter()
        .filter(|platform| **platform != here)
        .map(|platform| platform.as_str())
        .collect();
    let mut entry = model_entry(
        "elsewhere",
        vec![file_entry(
            "elsewhere.bin",
            &bytes,
            &[server.url("/elsewhere.bin")],
        )],
    );
    entry["platforms"] = json!(others);
    let root = TempDir::new("other-platform");
    let store = store(catalog(vec![entry]), root.path());

    let err = store
        .install(&id("elsewhere"), &ignore, &never())
        .unwrap_err();
    assert_eq!(err.kind, ErrorKind::Unsupported, "{err}");
    assert!(server.requests().is_empty(), "nothing was downloaded");
}
