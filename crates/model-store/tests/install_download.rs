mod support;

use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;

use dictation_api::ModelStore;
use dictation_types::{DownloadProgress, ModelInstallState};
use support::server::{Reply, TestServer};
use support::{TempDir, catalog, content, file_entry, id, ignore, model_entry, never, store};

#[test]
fn installing_downloads_every_file_and_marks_the_model_installed() {
    let server = TestServer::start();
    let first = content(5_000, 1);
    let second = content(3_333, 2);
    server.route("/first.bin", Reply::file(&first));
    server.route("/nested/second.bin", Reply::file(&second));
    let root = TempDir::new("install");
    let store = store(
        catalog(vec![model_entry(
            "two-files",
            vec![
                file_entry("first.bin", &first, &[server.url("/first.bin")]),
                file_entry(
                    "sub/second.bin",
                    &second,
                    &[server.url("/nested/second.bin")],
                ),
            ],
        )]),
        root.path(),
    );

    assert_eq!(
        store.get_status(&id("two-files")).unwrap().unwrap().state,
        ModelInstallState::NotInstalled
    );
    let status = store
        .install(&id("two-files"), &ignore, &never())
        .expect("installed");

    let folder = store
        .local_path(&id("two-files"))
        .unwrap()
        .expect("a folder");
    match &status.state {
        ModelInstallState::Installed {
            local_path,
            loaded_in_memory,
        } => {
            assert_eq!(PathBuf::from(local_path), folder);
            assert!(!loaded_in_memory);
        }
        other => panic!("not installed: {other:?}"),
    }
    assert_eq!(status.descriptor.id, id("two-files"));
    assert!(
        folder.starts_with(root.path()),
        "kept under the store's root"
    );
    assert_eq!(fs::read(folder.join("first.bin")).unwrap(), first);
    assert_eq!(fs::read(folder.join("sub/second.bin")).unwrap(), second);
    assert_eq!(
        store.get_status(&id("two-files")).unwrap().unwrap().state,
        status.state
    );
    assert_eq!(server.requests_for("/first.bin").len(), 1);
    assert_eq!(server.requests_for("/nested/second.bin").len(), 1);
}

#[test]
fn the_installed_model_is_listed_with_its_state() {
    let server = TestServer::start();
    let bytes = content(700, 3);
    server.route("/a.bin", Reply::file(&bytes));
    let root = TempDir::new("install-list");
    let store = store(
        catalog(vec![
            model_entry(
                "a",
                vec![file_entry("a.bin", &bytes, &[server.url("/a.bin")])],
            ),
            model_entry(
                "b",
                vec![file_entry("b.bin", &bytes, &[server.url("/a.bin")])],
            ),
        ]),
        root.path(),
    );
    store.install(&id("a"), &ignore, &never()).unwrap();
    let statuses = store.list_statuses().unwrap();
    assert_eq!(statuses.len(), 2);
    assert!(statuses[0].state.is_installed());
    assert_eq!(statuses[1].state, ModelInstallState::NotInstalled);
}

#[test]
fn progress_only_grows_ends_at_the_model_size_and_names_each_file() {
    let server = TestServer::start();
    let first = content(5_000, 4);
    let second = content(7_000, 5);
    server.route("/first.bin", Reply::file(&first));
    server.route("/second.bin", Reply::file(&second));
    let root = TempDir::new("progress");
    let store = store(
        catalog(vec![model_entry(
            "progress",
            vec![
                file_entry("first.bin", &first, &[server.url("/first.bin")]),
                file_entry("second.bin", &second, &[server.url("/second.bin")]),
            ],
        )]),
        root.path(),
    );
    let total = (first.len() + second.len()) as u64;
    let events: RefCell<Vec<DownloadProgress>> = RefCell::new(Vec::new());
    store
        .install(
            &id("progress"),
            &|event| events.borrow_mut().push(event.clone()),
            &never(),
        )
        .expect("installed");

    let events = events.into_inner();
    assert!(events.len() > 2, "progress was reported as it went");
    let mut previous = 0;
    for event in &events {
        assert_eq!(event.model_id, id("progress"));
        assert_eq!(event.total_bytes, total, "the total is the model size");
        assert!(event.downloaded_bytes >= previous, "progress went back");
        assert!(event.downloaded_bytes <= total, "progress passed the total");
        previous = event.downloaded_bytes;
        match event.file_path.as_str() {
            "first.bin" => assert!(event.downloaded_bytes <= first.len() as u64),
            "second.bin" => assert!(event.downloaded_bytes >= first.len() as u64),
            other => panic!("progress names an unknown file '{other}'"),
        }
    }
    assert_eq!(events.last().unwrap().downloaded_bytes, total);
    let first_of_second = events
        .iter()
        .position(|e| e.file_path == "second.bin")
        .expect("the second file is named");
    assert!(
        events[first_of_second..]
            .iter()
            .all(|e| e.file_path == "second.bin"),
        "files are fetched one after the other"
    );
}

#[test]
fn installing_an_installed_model_makes_no_request() {
    let server = TestServer::start();
    let bytes = content(2_000, 6);
    server.route("/m.bin", Reply::file(&bytes));
    let root = TempDir::new("reinstall");
    let store = store(
        catalog(vec![model_entry(
            "m",
            vec![file_entry("m.bin", &bytes, &[server.url("/m.bin")])],
        )]),
        root.path(),
    );
    let first = store.install(&id("m"), &ignore, &never()).unwrap();
    let asked = server.requests().len();

    let again = store.install(&id("m"), &ignore, &never()).unwrap();
    assert_eq!(server.requests().len(), asked, "nothing was asked for");
    assert_eq!(again.state, first.state);
}

#[test]
fn an_installed_model_is_still_installed_for_a_new_store_on_the_same_folder() {
    let server = TestServer::start();
    let bytes = content(1_500, 7);
    server.route("/m.bin", Reply::file(&bytes));
    let root = TempDir::new("reopen");
    let models = || {
        catalog(vec![model_entry(
            "m",
            vec![file_entry("m.bin", &bytes, &[server.url("/m.bin")])],
        )])
    };
    store(models(), root.path())
        .install(&id("m"), &ignore, &never())
        .unwrap();
    let asked = server.requests().len();

    let reopened = store(models(), root.path());
    assert!(reopened.local_path(&id("m")).unwrap().is_some());
    reopened.install(&id("m"), &ignore, &never()).unwrap();
    assert_eq!(server.requests().len(), asked);
}

#[test]
fn a_failed_download_leaves_the_model_not_installed() {
    let server = TestServer::start();
    let bytes = content(1_000, 8);
    server.route("/m.bin", Reply::Status(404));
    let root = TempDir::new("failed");
    let store = store(
        catalog(vec![model_entry(
            "m",
            vec![file_entry("m.bin", &bytes, &[server.url("/m.bin")])],
        )]),
        root.path(),
    );
    let err = store.install(&id("m"), &ignore, &never()).unwrap_err();
    assert_eq!(err.kind, dictation_types::ErrorKind::Network, "{err}");
    assert_eq!(
        store.get_status(&id("m")).unwrap().unwrap().state,
        ModelInstallState::NotInstalled
    );
    assert_eq!(store.local_path(&id("m")).unwrap(), None);
}
