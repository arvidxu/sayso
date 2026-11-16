mod support;

use std::fs::{self, OpenOptions};

use dictation_api::ModelStore;
use model_store::LocalModelStore;
use serde_json::{Value, json};
use support::archive::{file, folder, pax_global, pax_record, tar_bz2};
use support::server::{Reply, TestServer};
use support::{
    TempDir, catalog, content, file_entry, files_under, id, ignore, model_entry, never, store,
};

fn archive_entry(server: &TestServer, archive: &[u8]) -> Value {
    server.route("/pack.tar.bz2", Reply::file(archive));
    let mut entry = file_entry("pack.tar.bz2", archive, &[server.url("/pack.tar.bz2")]);
    entry["unpack"] = json!("tar.bz2");
    entry
}

fn installed(store: &LocalModelStore) -> bool {
    store
        .get_status(&id("packed"))
        .unwrap()
        .unwrap()
        .state
        .is_installed()
}

fn sample_archive() -> Vec<u8> {
    tar_bz2(&[
        folder("model/"),
        file("model/encoder.onnx", &content(3_000, 91)),
        file("model/tokens.txt", b"a\nb\nc\n"),
    ])
}

#[test]
fn deleting_an_unpacked_file_makes_the_model_not_installed() {
    let server = TestServer::start();
    let root = TempDir::new("records-delete");
    let entry = archive_entry(&server, &sample_archive());
    let store = store(
        catalog(vec![model_entry("packed", vec![entry])]),
        root.path(),
    );
    store.install(&id("packed"), &ignore, &never()).unwrap();
    let dir = store.local_path(&id("packed")).unwrap().unwrap();

    fs::remove_file(dir.join("model/tokens.txt")).unwrap();
    assert!(!installed(&store), "a missing unpacked file went unnoticed");
    assert_eq!(store.local_path(&id("packed")).unwrap(), None);
}

#[test]
fn shrinking_an_unpacked_file_makes_the_model_not_installed() {
    let server = TestServer::start();
    let root = TempDir::new("records-shrink");
    let entry = archive_entry(&server, &sample_archive());
    let store = store(
        catalog(vec![model_entry("packed", vec![entry])]),
        root.path(),
    );
    store.install(&id("packed"), &ignore, &never()).unwrap();
    let dir = store.local_path(&id("packed")).unwrap().unwrap();

    OpenOptions::new()
        .write(true)
        .open(dir.join("model/encoder.onnx"))
        .unwrap()
        .set_len(1_000)
        .unwrap();
    assert!(
        !installed(&store),
        "a shrunken unpacked file went unnoticed"
    );
}

#[test]
fn installing_again_after_an_unpacked_file_was_lost_restores_it() {
    let server = TestServer::start();
    let root = TempDir::new("records-restore");
    let entry = archive_entry(&server, &sample_archive());
    let store = store(
        catalog(vec![model_entry("packed", vec![entry])]),
        root.path(),
    );
    store.install(&id("packed"), &ignore, &never()).unwrap();
    let dir = store.local_path(&id("packed")).unwrap().unwrap();
    fs::remove_file(dir.join("model/encoder.onnx")).unwrap();

    store.install(&id("packed"), &ignore, &never()).unwrap();
    let dir = store.local_path(&id("packed")).unwrap().expect("installed");
    assert_eq!(
        fs::read(dir.join("model/encoder.onnx")).unwrap(),
        content(3_000, 91)
    );
}

#[test]
fn an_archive_entry_that_would_replace_a_catalog_file_is_refused() {
    let server = TestServer::start();
    let root = TempDir::new("records-overwrite");
    let config = content(400, 92);
    server.route("/config.json", Reply::file(&config));
    let archive = tar_bz2(&[
        file("model/encoder.onnx", b"weights"),
        file("config.json", b"replaced by the archive"),
    ]);
    let store = store(
        catalog(vec![model_entry(
            "packed",
            vec![
                file_entry("config.json", &config, &[server.url("/config.json")]),
                archive_entry(&server, &archive),
            ],
        )]),
        root.path(),
    );
    let result = store.install(&id("packed"), &ignore, &never());
    assert!(result.is_err(), "the archive replaced a catalog file");
    assert!(!installed(&store));
}

#[test]
fn leftovers_of_an_older_catalog_never_reach_the_installed_folder() {
    let server = TestServer::start();
    let root = TempDir::new("records-stale");
    let old_file = content(2_000, 93);
    let kept = content(8_000, 94);
    server.route("/old.bin", Reply::file(&old_file));
    server.route_in_turn(
        "/kept.bin",
        vec![
            Reply::file_cut_after(&kept, 3_000),
            Reply::Status(503),
            Reply::file(&kept),
        ],
    );

    // The older catalog's download stops after its first file and part of the second.
    let older = store(
        catalog(vec![model_entry(
            "packed",
            vec![
                file_entry("old.bin", &old_file, &[server.url("/old.bin")]),
                file_entry("kept.bin", &kept, &[server.url("/kept.bin")]),
            ],
        )]),
        root.path(),
    );
    older.install(&id("packed"), &ignore, &never()).unwrap_err();

    let newer = store(
        catalog(vec![model_entry(
            "packed",
            vec![file_entry("kept.bin", &kept, &[server.url("/kept.bin")])],
        )]),
        root.path(),
    );
    newer.install(&id("packed"), &ignore, &never()).unwrap();
    let dir = newer.local_path(&id("packed")).unwrap().unwrap();
    assert_eq!(fs::read(dir.join("kept.bin")).unwrap(), kept);
    let stale: Vec<String> = files_under(&dir)
        .into_iter()
        .filter(|(_, data)| data == &old_file)
        .map(|(path, _)| path)
        .collect();
    assert!(stale.is_empty(), "leftovers installed: {stale:?}");
    assert!(!dir.join("old.bin").exists());
}

#[test]
fn an_archive_with_a_pax_global_header_is_accepted() {
    let server = TestServer::start();
    let root = TempDir::new("records-pax");
    let records = pax_record("comment", "written by a test");
    let archive = tar_bz2(&[
        pax_global(&records),
        folder("model/"),
        file("model/encoder.onnx", b"weights"),
    ]);
    let store = store(
        catalog(vec![model_entry(
            "packed",
            vec![archive_entry(&server, &archive)],
        )]),
        root.path(),
    );
    store
        .install(&id("packed"), &ignore, &never())
        .expect("installed");
    let dir = store.local_path(&id("packed")).unwrap().unwrap();
    assert_eq!(
        fs::read(dir.join("model/encoder.onnx")).unwrap(),
        b"weights"
    );
    assert!(
        !dir.join("pax_global_header").exists(),
        "the header is not a file of the model"
    );
}
