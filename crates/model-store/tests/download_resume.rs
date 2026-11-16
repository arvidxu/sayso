mod support;

use std::cell::Cell;
use std::fs;

use dictation_api::ModelStore;
use dictation_types::{CancelToken, ErrorKind, ModelInstallState};
use model_store::LocalModelStore;
use support::server::{Reply, TestServer};
use support::{TempDir, catalog, content, file_entry, id, ignore, model_entry, never, store};

const LEN: usize = 64 * 1024;

fn one_file_store(server: &TestServer, bytes: &[u8], root: &TempDir) -> LocalModelStore {
    store(
        catalog(vec![model_entry(
            "big",
            vec![file_entry("big.bin", bytes, &[server.url("/big.bin")])],
        )]),
        root.path(),
    )
}

fn installed_bytes(store: &LocalModelStore) -> Vec<u8> {
    let folder = store.local_path(&id("big")).unwrap().expect("installed");
    fs::read(folder.join("big.bin")).unwrap()
}

#[test]
fn a_broken_connection_is_resumed_where_it_stopped() {
    let server = TestServer::start();
    let bytes = content(LEN, 11);
    let cut = 20_000;
    server.route_in_turn(
        "/big.bin",
        vec![
            Reply::file_cut_after(&bytes, cut),
            // Brings nothing, so the install ends there.
            Reply::Status(503),
            Reply::file(&bytes),
        ],
    );
    let root = TempDir::new("resume");
    let store = one_file_store(&server, &bytes, &root);

    let err = store.install(&id("big"), &ignore, &never()).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Network, "{err}");
    assert_eq!(
        store.get_status(&id("big")).unwrap().unwrap().state,
        ModelInstallState::NotInstalled
    );

    store
        .install(&id("big"), &ignore, &never())
        .expect("installed on the second try");
    let requests = server.requests_for("/big.bin");
    assert_eq!(requests.len(), 3);
    assert_eq!(
        requests[0].range, None,
        "the first try starts at the beginning"
    );
    assert_eq!(requests[2].range.as_deref(), Some("bytes=20000-"));
    assert_eq!(requests[2].status, 206);
    assert_eq!(requests[2].body_bytes_sent, LEN - cut, "only the rest");
    assert_eq!(installed_bytes(&store), bytes);
}

#[test]
fn progress_of_a_resumed_download_starts_from_what_was_kept() {
    let server = TestServer::start();
    let bytes = content(LEN, 12);
    server.route_in_turn(
        "/big.bin",
        vec![
            Reply::file_cut_after(&bytes, 30_000),
            // Brings nothing, so the install ends there.
            Reply::Status(503),
            Reply::file(&bytes),
        ],
    );
    let root = TempDir::new("resume-progress");
    let store = one_file_store(&server, &bytes, &root);
    store.install(&id("big"), &ignore, &never()).unwrap_err();

    let lowest = Cell::new(u64::MAX);
    store
        .install(
            &id("big"),
            &|event| lowest.set(lowest.get().min(event.downloaded_bytes)),
            &never(),
        )
        .unwrap();
    assert_eq!(lowest.get(), 30_000);
}

#[test]
fn a_server_that_ignores_the_range_still_gives_the_same_file() {
    let server = TestServer::start();
    let bytes = content(LEN, 13);
    server.route_in_turn(
        "/big.bin",
        vec![
            Reply::file_cut_after(&bytes, 25_000),
            // Brings nothing, so the install ends there.
            Reply::Status(503),
            Reply::file_without_ranges(&bytes),
        ],
    );
    let root = TempDir::new("no-ranges");
    let store = one_file_store(&server, &bytes, &root);
    store.install(&id("big"), &ignore, &never()).unwrap_err();

    store
        .install(&id("big"), &ignore, &never())
        .expect("installed from the whole file");
    let requests = server.requests_for("/big.bin");
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[2].range.as_deref(), Some("bytes=25000-"));
    assert_eq!(requests[2].status, 200);
    assert_eq!(installed_bytes(&store), bytes);
}

#[test]
fn cancelling_stops_the_download_and_installing_again_resumes_it() {
    let server = TestServer::start();
    let bytes = content(LEN, 14);
    server.route("/big.bin", Reply::file(&bytes));
    let root = TempDir::new("cancel");
    let store = one_file_store(&server, &bytes, &root);

    let cancel = CancelToken::new();
    let kept = Cell::new(0u64);
    let err = store
        .install(
            &id("big"),
            &|event| {
                if !cancel.is_cancelled() {
                    kept.set(event.downloaded_bytes);
                    if event.downloaded_bytes >= 10_000 {
                        cancel.cancel();
                    }
                }
            },
            &cancel,
        )
        .unwrap_err();
    assert_eq!(err.kind, ErrorKind::Cancelled, "{err}");
    let kept = kept.get();
    assert!(
        (10_000..LEN as u64).contains(&kept),
        "stopped part way: {kept}"
    );
    assert_eq!(
        store.get_status(&id("big")).unwrap().unwrap().state,
        ModelInstallState::NotInstalled
    );

    store
        .install(&id("big"), &ignore, &never())
        .expect("installed after the cancel");
    let requests = server.requests_for("/big.bin");
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[1].range.as_deref(),
        Some(format!("bytes={kept}-").as_str())
    );
    assert_eq!(installed_bytes(&store), bytes);
}

#[test]
fn a_token_cancelled_before_installing_stops_without_installing() {
    let server = TestServer::start();
    let bytes = content(4_000, 15);
    server.route("/big.bin", Reply::file(&bytes));
    let root = TempDir::new("cancel-early");
    let store = one_file_store(&server, &bytes, &root);
    let cancel = CancelToken::new();
    cancel.cancel();
    let err = store.install(&id("big"), &ignore, &cancel).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Cancelled, "{err}");
    assert_eq!(store.local_path(&id("big")).unwrap(), None);
}
