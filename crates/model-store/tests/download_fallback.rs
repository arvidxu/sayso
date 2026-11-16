mod support;

use std::fs;

use dictation_api::ModelStore;
use dictation_types::{ErrorKind, ModelInstallState};
use model_store::LocalModelStore;
use support::server::{Reply, TestServer, unreachable_url};
use support::{
    TempDir, catalog, content, file_entry, files_under, id, ignore, model_entry, never, store,
};

fn store_with_urls(bytes: &[u8], urls: &[String], root: &TempDir) -> LocalModelStore {
    store(
        catalog(vec![model_entry(
            "m",
            vec![file_entry("m.bin", bytes, urls)],
        )]),
        root.path(),
    )
}

fn installed_bytes(store: &LocalModelStore) -> Vec<u8> {
    let folder = store.local_path(&id("m")).unwrap().expect("installed");
    fs::read(folder.join("m.bin")).unwrap()
}

#[test]
fn an_unreachable_first_address_falls_back_to_the_next() {
    let server = TestServer::start();
    let bytes = content(3_000, 21);
    server.route("/m.bin", Reply::file(&bytes));
    let root = TempDir::new("unreachable");
    let store = store_with_urls(
        &bytes,
        &[unreachable_url("/m.bin"), server.url("/m.bin")],
        &root,
    );
    store
        .install(&id("m"), &ignore, &never())
        .expect("installed");
    assert_eq!(installed_bytes(&store), bytes);
}

#[test]
fn an_address_answering_not_found_or_a_server_error_falls_back_to_the_next() {
    for status in [404, 500] {
        let server = TestServer::start();
        let bytes = content(3_000, 22);
        server.route("/broken.bin", Reply::Status(status));
        server.route("/m.bin", Reply::file(&bytes));
        let root = TempDir::new("status");
        let store = store_with_urls(
            &bytes,
            &[server.url("/broken.bin"), server.url("/m.bin")],
            &root,
        );
        store
            .install(&id("m"), &ignore, &never())
            .unwrap_or_else(|err| panic!("after {status}: {err}"));
        assert_eq!(server.requests_for("/broken.bin").len(), 1);
        assert_eq!(server.requests_for("/m.bin").len(), 1);
        assert_eq!(installed_bytes(&store), bytes);
    }
}

#[test]
fn when_every_address_fails_the_install_fails_with_a_network_error() {
    let server = TestServer::start();
    let bytes = content(3_000, 23);
    server.route("/a.bin", Reply::Status(500));
    let root = TempDir::new("all-fail");
    let store = store_with_urls(
        &bytes,
        &[server.url("/a.bin"), unreachable_url("/b.bin")],
        &root,
    );
    let err = store.install(&id("m"), &ignore, &never()).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Network, "{err}");
    assert_eq!(store.local_path(&id("m")).unwrap(), None);
}

#[test]
fn wrong_bytes_at_the_first_address_fall_back_to_the_right_bytes_at_the_next() {
    let server = TestServer::start();
    let bytes = content(4_000, 24);
    let wrong = content(4_000, 25);
    server.route("/bad.bin", Reply::file(&wrong));
    server.route("/good.bin", Reply::file(&bytes));
    let root = TempDir::new("wrong-first");
    let store = store_with_urls(
        &bytes,
        &[server.url("/bad.bin"), server.url("/good.bin")],
        &root,
    );
    store
        .install(&id("m"), &ignore, &never())
        .expect("installed");
    assert_eq!(installed_bytes(&store), bytes);
    let good = server.requests_for("/good.bin");
    assert_eq!(good.len(), 1);
    assert_eq!(good[0].range, None, "the wrong bytes were not continued");
}

#[test]
fn wrong_bytes_everywhere_fail_with_a_checksum_mismatch_and_are_not_kept() {
    let server = TestServer::start();
    let bytes = content(4_000, 26);
    let wrong = content(4_000, 27);
    let other_wrong = content(4_000, 28);
    server.route("/a.bin", Reply::file(&wrong));
    server.route("/b.bin", Reply::file(&other_wrong));
    let root = TempDir::new("wrong-everywhere");
    let store = store_with_urls(&bytes, &[server.url("/a.bin"), server.url("/b.bin")], &root);

    let err = store.install(&id("m"), &ignore, &never()).unwrap_err();
    assert_eq!(err.kind, ErrorKind::ChecksumMismatch, "{err}");
    assert!(
        !store
            .get_status(&id("m"))
            .unwrap()
            .unwrap()
            .state
            .is_installed()
    );
    assert_eq!(store.local_path(&id("m")).unwrap(), None);
    for (path, kept) in files_under(root.path()) {
        assert!(
            kept != wrong && kept != other_wrong,
            "'{path}' keeps the wrong content"
        );
    }

    // The addresses now serve the right file: it is fetched from the start.
    server.route("/a.bin", Reply::file(&bytes));
    let asked = server.requests().len();
    store
        .install(&id("m"), &ignore, &never())
        .expect("installed once the right file is served");
    let again = &server.requests()[asked..];
    assert_eq!(again.len(), 1);
    assert_eq!(again[0].path, "/a.bin");
    assert_eq!(again[0].range, None, "nothing wrong was resumed");
    assert_eq!(installed_bytes(&store), bytes);
    assert!(matches!(
        store.get_status(&id("m")).unwrap().unwrap().state,
        ModelInstallState::Installed { .. }
    ));
}
