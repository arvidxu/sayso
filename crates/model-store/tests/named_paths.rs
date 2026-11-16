mod support;

use std::fs;

use dictation_api::ModelStore;
use dictation_types::ErrorKind;
use model_store::{LocalModelStore, ModelCatalog};
use serde_json::{Value, json};
use support::archive::{file, folder, tar_bz2};
use support::server::{Reply, TestServer};
use support::{
    TempDir, catalog, catalog_json, content, file_entry, id, ignore, model_entry, never, store,
};

fn archive_model(server: &TestServer, named: Value) -> Value {
    let archive = tar_bz2(&[
        folder("model/"),
        file("model/encoder.onnx", &content(2_000, 131)),
        file("model/tokens.txt", b"a\nb\n"),
    ]);
    server.route("/pack.tar.bz2", Reply::file(&archive));
    let mut archive_file = file_entry("pack.tar.bz2", &archive, &[server.url("/pack.tar.bz2")]);
    archive_file["unpack"] = json!("tar.bz2");
    let mut entry = model_entry("named", vec![archive_file]);
    entry["namedPaths"] = named;
    entry
}

fn installed(store: &LocalModelStore) -> bool {
    store
        .get_status(&id("named"))
        .unwrap()
        .unwrap()
        .state
        .is_installed()
}

fn refused(named: Value) -> bool {
    let mut entry = model_entry(
        "m",
        vec![file_entry(
            "model.bin",
            b"data",
            &["https://example.invalid/model.bin".to_owned()],
        )],
    );
    entry["namedPaths"] = named;
    match ModelCatalog::from_json(&catalog_json(vec![entry])) {
        Ok(_) => false,
        Err(err) => {
            assert_eq!(err.kind, ErrorKind::Validation, "{err}");
            true
        }
    }
}

#[test]
fn named_paths_are_read_from_the_catalog() {
    let catalog = catalog(vec![{
        let mut entry = model_entry(
            "m",
            vec![file_entry(
                "model.bin",
                b"data",
                &["https://example.invalid/model.bin".to_owned()],
            )],
        );
        entry["namedPaths"] = json!({ "tokens": "model/tokens.txt", "encoder": "model.bin" });
        entry
    }]);
    let mut named = catalog.get(&id("m")).unwrap().named_paths.clone();
    named.sort();
    assert_eq!(
        named,
        [
            ("encoder".to_owned(), "model.bin".to_owned()),
            ("tokens".to_owned(), "model/tokens.txt".to_owned())
        ]
    );
}

#[test]
fn an_entry_without_named_paths_has_none() {
    let catalog = catalog(vec![model_entry(
        "m",
        vec![file_entry(
            "model.bin",
            b"data",
            &["https://example.invalid/model.bin".to_owned()],
        )],
    )]);
    assert!(catalog.get(&id("m")).unwrap().named_paths.is_empty());
}

#[test]
fn a_model_whose_named_paths_exist_is_installed() {
    let server = TestServer::start();
    let root = TempDir::new("named");
    let entry = archive_model(
        &server,
        json!({ "encoder": "model/encoder.onnx", "tokens": "model/tokens.txt", "folder": "model" }),
    );
    let store = store(catalog(vec![entry]), root.path());
    store
        .install(&id("named"), &ignore, &never())
        .expect("installed");
    assert!(installed(&store));
}

#[test]
fn a_model_missing_a_named_path_does_not_count_as_installed() {
    let server = TestServer::start();
    let root = TempDir::new("named-missing");
    let entry = archive_model(&server, json!({ "decoder": "model/decoder.onnx" }));
    let store = store(catalog(vec![entry]), root.path());
    let _ = store.install(&id("named"), &ignore, &never());
    assert!(
        !installed(&store),
        "a model without its decoder counts as installed"
    );
    assert_eq!(store.local_path(&id("named")).unwrap(), None);
}

#[test]
fn removing_a_named_path_after_installing_makes_the_model_not_installed() {
    let server = TestServer::start();
    let root = TempDir::new("named-removed");
    let entry = archive_model(&server, json!({ "encoder": "model/encoder.onnx" }));
    let store = store(catalog(vec![entry]), root.path());
    store.install(&id("named"), &ignore, &never()).unwrap();
    let dir = store.local_path(&id("named")).unwrap().unwrap();
    fs::remove_file(dir.join("model/encoder.onnx")).unwrap();
    assert!(!installed(&store));
}

#[test]
fn invalid_names_are_refused() {
    for name in ["", " ", " encoder", "encoder "] {
        assert!(refused(json!({ name: "model.bin" })), "the name {name:?}");
    }
}

#[test]
fn the_same_name_twice_is_refused() {
    let entry = model_entry(
        "m",
        vec![file_entry(
            "model.bin",
            b"data",
            &["https://example.invalid/model.bin".to_owned()],
        )],
    );
    let mut text = catalog_json(vec![entry]);
    text = text.replacen(
        "\"files\"",
        "\"namedPaths\":{\"encoder\":\"model.bin\",\"encoder\":\"model.bin\"},\"files\"",
        1,
    );
    assert!(ModelCatalog::from_json(&text).is_err());
}

#[test]
fn named_paths_that_are_not_safe_relative_paths_are_refused() {
    for path in [
        "",
        "/model.bin",
        "../model.bin",
        "model/../../x",
        "model\\encoder.onnx",
        "C:/model.bin",
        "model//encoder.onnx",
    ] {
        assert!(refused(json!({ "encoder": path })), "the path {path:?}");
    }
}

#[test]
fn named_paths_that_are_not_strings_are_refused() {
    assert!(refused(json!({ "encoder": 3 })));
    assert!(refused(json!(["model.bin"])));
}
