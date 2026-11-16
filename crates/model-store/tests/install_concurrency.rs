mod support;

use std::fs;
use std::thread;
use std::time::{Duration, Instant};

use dictation_api::ModelStore;
use dictation_types::{ErrorKind, ModelInstallState};
use support::server::{Reply, TestServer};
use support::{TempDir, catalog, content, file_entry, id, ignore, model_entry, never, store};

const LEN: usize = 32 * 1024;

fn wait_for(what: &str, done: impl Fn() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "timed out: {what}"
        );
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn a_second_install_of_the_same_model_is_refused_while_the_first_runs() {
    let server = TestServer::start();
    let bytes = content(LEN, 31);
    server.route(
        "/slow.bin",
        Reply::file_slowly(&bytes, 1024, Duration::from_millis(25)),
    );
    let root = TempDir::new("concurrent");
    let source = TempDir::new("concurrent-source");
    fs::write(source.join("slow.bin"), &bytes).unwrap();
    let store = store(
        catalog(vec![model_entry(
            "slow",
            vec![file_entry("slow.bin", &bytes, &[server.url("/slow.bin")])],
        )]),
        root.path(),
    );

    thread::scope(|scope| {
        let first = scope.spawn(|| store.install(&id("slow"), &ignore, &never()));
        wait_for("the first download started", || {
            !server.requests_for("/slow.bin").is_empty()
        });

        let state = store.get_status(&id("slow")).unwrap().unwrap().state;
        assert!(
            matches!(state, ModelInstallState::Downloading { total_bytes, .. }
                if total_bytes == LEN as u64),
            "{state:?}"
        );

        let err = store
            .install(&id("slow"), &ignore, &never())
            .expect_err("the second install is refused");
        assert_eq!(err.kind, ErrorKind::AlreadyExists, "{err}");

        let err = store
            .import_local(&id("slow"), source.path())
            .expect_err("importing is refused too");
        assert_eq!(err.kind, ErrorKind::AlreadyExists, "{err}");

        let err = store
            .delete(&id("slow"))
            .expect_err("deleting is refused while downloading");
        assert_eq!(err.kind, ErrorKind::AlreadyExists, "{err}");

        let status = first.join().unwrap().expect("the first install finished");
        assert!(status.state.is_installed());
    });

    assert_eq!(server.requests_for("/slow.bin").len(), 1, "fetched once");
    let folder = store.local_path(&id("slow")).unwrap().unwrap();
    assert_eq!(fs::read(folder.join("slow.bin")).unwrap(), bytes);
}

#[test]
fn different_models_can_be_installed_at_the_same_time() {
    let server = TestServer::start();
    let a = content(8 * 1024, 32);
    let b = content(8 * 1024, 33);
    server.route(
        "/a.bin",
        Reply::file_slowly(&a, 1024, Duration::from_millis(10)),
    );
    server.route(
        "/b.bin",
        Reply::file_slowly(&b, 1024, Duration::from_millis(10)),
    );
    let root = TempDir::new("parallel");
    let store = store(
        catalog(vec![
            model_entry("a", vec![file_entry("a.bin", &a, &[server.url("/a.bin")])]),
            model_entry("b", vec![file_entry("b.bin", &b, &[server.url("/b.bin")])]),
        ]),
        root.path(),
    );
    thread::scope(|scope| {
        let first = scope.spawn(|| store.install(&id("a"), &ignore, &never()));
        let second = scope.spawn(|| store.install(&id("b"), &ignore, &never()));
        first.join().unwrap().expect("a installed");
        second.join().unwrap().expect("b installed");
    });
    assert!(store.local_path(&id("a")).unwrap().is_some());
    assert!(store.local_path(&id("b")).unwrap().is_some());
}
