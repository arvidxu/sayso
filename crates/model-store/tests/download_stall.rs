mod support;

use std::thread;
use std::time::{Duration, Instant};

use dictation_api::ModelStore;
use dictation_types::{CancelToken, ErrorKind};
use serde_json::json;
use support::server::{Body, TestServer};
use support::{
    TempDir, catalog, content, file_entry, id, ignore, model_entry, store_tuned, test_overrides,
    tuning_with,
};

#[test]
fn cancelling_during_a_stalled_connection_is_reported_as_cancelled() {
    let server = TestServer::start();
    let bytes = content(32 * 1024, 111);
    server.route(
        "/m.bin",
        Body::new(&bytes)
            .stall_after(4_096, Duration::from_secs(30))
            .reply(),
    );
    let root = TempDir::new("stall");
    let mut overrides = test_overrides(1024);
    overrides["readTimeoutMs"] = json!(1_500);
    let store = store_tuned(
        catalog(vec![model_entry(
            "m",
            vec![file_entry("m.bin", &bytes, &[server.url("/m.bin")])],
        )]),
        root.path(),
        &tuning_with(overrides),
    );

    let cancel = CancelToken::new();
    let started = Instant::now();
    let result = thread::scope(|scope| {
        let install = scope.spawn(|| store.install(&id("m"), &ignore, &cancel));
        // Wait until the server has sent what it sends before stalling.
        while server.requests_for("/m.bin").is_empty() {
            thread::sleep(Duration::from_millis(5));
        }
        thread::sleep(Duration::from_millis(300));
        cancel.cancel();
        install.join().unwrap()
    });
    let err = result.expect_err("the install was cancelled");
    assert_eq!(err.kind, ErrorKind::Cancelled, "{err}");
    assert!(
        started.elapsed() < Duration::from_secs(15),
        "the cancel waited for the stalled server"
    );
    assert_eq!(store.local_path(&id("m")).unwrap(), None);
}
