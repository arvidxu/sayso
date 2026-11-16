mod support;

use std::fs;

use dictation_api::ModelStore;
use dictation_types::ErrorKind;
use model_store::LocalModelStore;
use serde_json::json;
use support::server::{Body, Reply, TestServer, unreachable_url};
use support::{
    TempDir, catalog, content, file_entry, id, ignore, model_entry, never, store_tuned, tuning_with,
};

const LEN: usize = 32 * 1024;

fn retrying_store(urls: &[String], bytes: &[u8], attempts: u32, root: &TempDir) -> LocalModelStore {
    let tuning = tuning_with(json!({
        "connectTimeoutMs": 2_000,
        "readTimeoutMs": 20_000,
        "bufferBytes": 1024,
        "attemptsPerAddress": attempts,
        "retryDelayMs": 0,
    }));
    store_tuned(
        catalog(vec![model_entry(
            "m",
            vec![file_entry("m.bin", bytes, urls)],
        )]),
        root.path(),
        &tuning,
    )
}

fn installed_bytes(store: &LocalModelStore) -> Vec<u8> {
    let folder = store.local_path(&id("m")).unwrap().expect("installed");
    fs::read(folder.join("m.bin")).unwrap()
}

#[test]
fn a_dropped_connection_is_retried_at_the_same_address_as_configured() {
    let server = TestServer::start();
    let bytes = content(LEN, 101);
    server.route_in_turn(
        "/m.bin",
        vec![
            Reply::file_cut_after(&bytes, 5_000),
            Reply::file_cut_after(&bytes, 6_000),
            Reply::file(&bytes),
        ],
    );
    let root = TempDir::new("retry-same");
    let store = retrying_store(&[server.url("/m.bin")], &bytes, 3, &root);

    store
        .install(&id("m"), &ignore, &never())
        .expect("installed within the attempts");
    let ranges: Vec<Option<String>> = server
        .requests_for("/m.bin")
        .into_iter()
        .map(|r| r.range)
        .collect();
    assert_eq!(
        ranges,
        [
            None,
            Some("bytes=5000-".to_owned()),
            Some("bytes=11000-".to_owned())
        ]
    );
    assert_eq!(installed_bytes(&store), bytes);
}

#[test]
fn after_the_attempts_at_one_address_the_next_address_is_used() {
    let server = TestServer::start();
    let bytes = content(LEN, 102);
    // After bringing part of the file, the first address only fails.
    server.route_in_turn(
        "/first.bin",
        vec![Reply::file_cut_after(&bytes, 4_000), Reply::Status(503)],
    );
    server.route("/second.bin", Reply::file(&bytes));
    let root = TempDir::new("retry-next");
    let store = retrying_store(
        &[server.url("/first.bin"), server.url("/second.bin")],
        &bytes,
        2,
        &root,
    );
    store
        .install(&id("m"), &ignore, &never())
        .expect("installed");
    // The try that brought part of the file, then two that brought nothing.
    assert_eq!(server.requests_for("/first.bin").len(), 3);
    let second = server.requests_for("/second.bin");
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].range.as_deref(), Some("bytes=4000-"));
    assert_eq!(installed_bytes(&store), bytes);
}

#[test]
fn an_unreachable_address_is_not_retried_forever() {
    let server = TestServer::start();
    let bytes = content(2_000, 103);
    server.route("/m.bin", Reply::file(&bytes));
    let root = TempDir::new("retry-unreachable");
    let store = retrying_store(
        &[unreachable_url("/m.bin"), server.url("/m.bin")],
        &bytes,
        3,
        &root,
    );
    store
        .install(&id("m"), &ignore, &never())
        .expect("installed");
    assert_eq!(installed_bytes(&store), bytes);
}

#[test]
fn a_resumed_file_that_does_not_match_is_fetched_again_from_the_start() {
    let server = TestServer::start();
    let bytes = content(LEN, 104);
    let wrong = content(LEN, 105);
    server.route_in_turn(
        "/m.bin",
        vec![
            Reply::file_cut_after(&wrong, 10_000),
            Reply::file(&bytes),
            Reply::file(&bytes),
        ],
    );
    let root = TempDir::new("retry-mismatch");
    let store = retrying_store(&[server.url("/m.bin")], &bytes, 2, &root);

    store
        .install(&id("m"), &ignore, &never())
        .expect("installed after starting over");
    let requests = server.requests_for("/m.bin");
    assert_eq!(requests.len(), 3, "{requests:?}");
    assert_eq!(requests[1].range.as_deref(), Some("bytes=10000-"));
    assert_eq!(requests[2].range, None, "started over from zero");
    assert_eq!(installed_bytes(&store), bytes);
}

#[test]
fn a_range_the_server_cannot_satisfy_starts_the_file_over() {
    let server = TestServer::start();
    let bytes = content(LEN, 106);
    server.route_in_turn(
        "/m.bin",
        vec![
            Reply::file_cut_after(&bytes, 7_000),
            // Brings nothing, so the install ends there.
            Reply::Status(503),
            Reply::Status(416),
            Reply::file(&bytes),
        ],
    );
    let root = TempDir::new("retry-416");
    let store = retrying_store(&[server.url("/m.bin")], &bytes, 1, &root);
    store.install(&id("m"), &ignore, &never()).unwrap_err();

    store
        .install(&id("m"), &ignore, &never())
        .expect("installed after a 416");
    let requests = server.requests_for("/m.bin");
    assert_eq!(requests[2].range.as_deref(), Some("bytes=7000-"));
    assert_eq!(requests[2].status, 416);
    assert_eq!(requests.len(), 4, "{requests:?}");
    assert_eq!(requests[3].range, None, "started over from zero");
    assert_eq!(installed_bytes(&store), bytes);
}

#[test]
fn a_partial_answer_starting_at_the_wrong_byte_starts_the_file_over() {
    let server = TestServer::start();
    let bytes = content(LEN, 107);
    server.route_in_turn(
        "/m.bin",
        vec![
            Reply::file_cut_after(&bytes, 9_000),
            // Brings nothing, so the install ends there.
            Reply::Status(503),
            Body::new(&bytes).wrong_offset().reply(),
            Reply::file(&bytes),
        ],
    );
    let root = TempDir::new("retry-offset");
    let store = retrying_store(&[server.url("/m.bin")], &bytes, 1, &root);
    store.install(&id("m"), &ignore, &never()).unwrap_err();

    store
        .install(&id("m"), &ignore, &never())
        .expect("installed after a misplaced partial answer");
    assert_eq!(installed_bytes(&store), bytes);
}

#[test]
fn a_server_claiming_another_total_size_fails_that_address() {
    let server = TestServer::start();
    let bytes = content(LEN, 108);
    server.route_in_turn(
        "/first.bin",
        vec![
            Reply::file_cut_after(&bytes, 6_000),
            // Brings nothing, so the install ends there.
            Reply::Status(503),
            Body::new(&bytes).claimed_total(LEN + 1).reply(),
        ],
    );
    server.route_in_turn("/second.bin", vec![Reply::Status(500), Reply::file(&bytes)]);
    let root = TempDir::new("retry-total");
    let store = retrying_store(
        &[server.url("/first.bin"), server.url("/second.bin")],
        &bytes,
        1,
        &root,
    );
    let err = store.install(&id("m"), &ignore, &never()).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Network, "{err}");

    store
        .install(&id("m"), &ignore, &never())
        .expect("installed from the second address");
    let first = server.requests_for("/first.bin");
    assert_eq!(first.len(), 3);
    assert_eq!(first[2].range.as_deref(), Some("bytes=6000-"));
    let second = server.requests_for("/second.bin");
    assert_eq!(second.len(), 2, "the second address was used: {second:?}");
    assert_eq!(installed_bytes(&store), bytes);
}

#[test]
fn a_server_that_drops_every_connection_still_delivers_the_file_piece_by_piece() {
    let server = TestServer::start();
    let bytes = content(LEN, 109);
    // Every connection breaks off after 4 000 bytes: nine tries for the whole file, far more
    // than the three tries allowed for failures in a row.
    server.route("/m.bin", Reply::file_cut_after(&bytes, 4_000));
    let root = TempDir::new("retry-progress");
    let store = retrying_store(&[server.url("/m.bin")], &bytes, 3, &root);

    store
        .install(&id("m"), &ignore, &never())
        .expect("installed piece by piece");
    assert_eq!(server.requests_for("/m.bin").len(), LEN.div_ceil(4_000));
    assert_eq!(installed_bytes(&store), bytes);
}

#[test]
fn fetching_again_what_was_thrown_away_does_not_keep_the_tries_going() {
    let server = TestServer::start();
    let bytes = content(LEN, 110);
    // Every other answer makes the partial file start over, so the file never gets further
    // than the first piece.
    let cut = Reply::file_cut_after(&bytes, 4_000);
    let misplaced = Body::new(&bytes).wrong_offset().reply();
    let mut replies = Vec::new();
    for _ in 0..20 {
        replies.push(cut.clone());
        replies.push(misplaced.clone());
    }
    server.route_in_turn("/m.bin", replies);
    let root = TempDir::new("retry-no-progress");
    let store = retrying_store(&[server.url("/m.bin")], &bytes, 2, &root);

    let err = store.install(&id("m"), &ignore, &never()).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Network, "{err}");
    // The first piece, a start over that is not counted, the same piece again (no progress,
    // one failure), and a counted start over (two failures).
    assert_eq!(server.requests_for("/m.bin").len(), 4);
}
