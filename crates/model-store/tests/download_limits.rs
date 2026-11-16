mod support;

use std::fs;

use dictation_api::ModelStore;
use model_store::LocalModelStore;
use serde_json::{Value, json};
use support::server::{Reply, TestServer};
use support::{TempDir, catalog, content, file_entry, id, ignore, model_entry, never, store_tuned};

const LEN: usize = 32 * 1024;
/// Every connection of a breaking address ends after this many bytes.
const PIECE: usize = 4_000;

fn store_with(urls: &[String], bytes: &[u8], root: &TempDir, overrides: Value) -> LocalModelStore {
    let mut settings = json!({
        "connectTimeoutMs": 2_000,
        "readTimeoutMs": 20_000,
        "bufferBytes": 1024,
        "retryDelayMs": 0,
    });
    for (key, value) in overrides.as_object().unwrap() {
        settings[key] = value.clone();
    }
    store_tuned(
        catalog(vec![model_entry(
            "m",
            vec![file_entry("m.bin", bytes, urls)],
        )]),
        root.path(),
        &support::tuning_with(settings),
    )
}

fn installed_bytes(store: &LocalModelStore) -> Vec<u8> {
    let folder = store.local_path(&id("m")).unwrap().expect("installed");
    fs::read(folder.join("m.bin")).unwrap()
}

#[test]
fn a_leftover_partial_file_longer_than_the_file_does_not_stop_progress_from_counting() {
    let server = TestServer::start();
    let bytes = content(LEN, 121);
    server.route("/m.bin", Reply::file_cut_after(&bytes, PIECE));
    let root = TempDir::new("limits-oversized");
    let downloads = root.join("partial").join("m").join("downloads");
    fs::create_dir_all(&downloads).unwrap();
    fs::write(downloads.join("m.bin.part"), vec![7u8; LEN * 2]).unwrap();
    let store = store_with(
        &[server.url("/m.bin")],
        &bytes,
        &root,
        json!({ "attemptsPerAddress": 2 }),
    );

    store
        .install(&id("m"), &ignore, &never())
        .expect("installed piece by piece");
    assert_eq!(server.requests_for("/m.bin").len(), LEN.div_ceil(PIECE));
    assert_eq!(installed_bytes(&store), bytes);
}
