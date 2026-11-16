mod support;

use std::fs;

use dictation_api::ModelStore;
use dictation_types::{ErrorKind, ModelInstallState};
use model_store::LocalModelStore;
use support::server::{Reply, TestServer};
use support::{TempDir, catalog, content, file_entry, id, ignore, model_entry, never, store};

const LEN: usize = 16 * 1024;

fn one_model(server: &TestServer, bytes: &[u8], root: &TempDir) -> LocalModelStore {
    store(
        catalog(vec![model_entry(
            "m",
            vec![file_entry("m.bin", bytes, &[server.url("/m.bin")])],
        )]),
        root.path(),
    )
}

#[test]
fn deleting_an_installed_model_removes_it_and_says_it_was_installed() {
    let server = TestServer::start();
    let bytes = content(LEN, 61);
    server.route("/m.bin", Reply::file(&bytes));
    let root = TempDir::new("delete");
    let store = one_model(&server, &bytes, &root);
    store.install(&id("m"), &ignore, &never()).unwrap();
    let folder = store.local_path(&id("m")).unwrap().unwrap();
    assert!(folder.join("m.bin").is_file());

    assert!(store.delete(&id("m")).unwrap(), "it was installed");
    assert!(!folder.exists(), "the model's folder is gone");
    assert_eq!(store.local_path(&id("m")).unwrap(), None);
    assert_eq!(
        store.get_status(&id("m")).unwrap().unwrap().state,
        ModelInstallState::NotInstalled
    );

    assert!(!store.delete(&id("m")).unwrap(), "nothing left to delete");
}

#[test]
fn installing_after_deleting_downloads_the_model_again() {
    let server = TestServer::start();
    let bytes = content(LEN, 62);
    server.route("/m.bin", Reply::file(&bytes));
    let root = TempDir::new("delete-reinstall");
    let store = one_model(&server, &bytes, &root);
    store.install(&id("m"), &ignore, &never()).unwrap();
    store.delete(&id("m")).unwrap();
    store.install(&id("m"), &ignore, &never()).unwrap();
    let requests = server.requests_for("/m.bin");
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[1].range, None);
}

#[test]
fn deleting_removes_an_unfinished_download() {
    let server = TestServer::start();
    let bytes = content(LEN, 63);
    server.route_in_turn(
        "/m.bin",
        vec![
            Reply::file_cut_after(&bytes, 6_000),
            // Brings nothing, so the install ends there.
            Reply::Status(503),
            Reply::file(&bytes),
        ],
    );
    let root = TempDir::new("delete-partial");
    let store = one_model(&server, &bytes, &root);
    store.install(&id("m"), &ignore, &never()).unwrap_err();

    assert!(!store.delete(&id("m")).unwrap(), "it was not installed");
    let remaining: Vec<String> = support::files_under(root.path())
        .into_iter()
        .map(|(path, _)| path)
        .collect();
    assert!(remaining.is_empty(), "left behind: {remaining:?}");

    store.install(&id("m"), &ignore, &never()).unwrap();
    let requests = server.requests_for("/m.bin");
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[2].range, None, "nothing was kept to resume");
}

#[test]
fn deleting_a_model_clears_its_corrupted_mark() {
    let root = TempDir::new("delete-corrupted");
    let bytes = content(500, 64);
    let store = store(
        catalog(vec![model_entry(
            "m",
            vec![file_entry(
                "m.bin",
                &bytes,
                &["https://example.invalid/m.bin".to_owned()],
            )],
        )]),
        root.path(),
    );
    let source = TempDir::new("delete-corrupted-source");
    fs::write(source.join("m.bin"), content(500, 65)).unwrap();
    store.import_local(&id("m"), source.path()).unwrap_err();
    assert!(matches!(
        store.get_status(&id("m")).unwrap().unwrap().state,
        ModelInstallState::Corrupted { .. }
    ));
    assert!(!store.delete(&id("m")).unwrap());
    assert_eq!(
        store.get_status(&id("m")).unwrap().unwrap().state,
        ModelInstallState::NotInstalled
    );
}

#[test]
fn the_local_path_is_given_only_while_the_model_is_installed() {
    let server = TestServer::start();
    let bytes = content(LEN, 66);
    server.route_in_turn(
        "/m.bin",
        vec![
            Reply::file_cut_after(&bytes, 4_000),
            // Brings nothing, so the install ends there.
            Reply::Status(503),
            Reply::file(&bytes),
        ],
    );
    let root = TempDir::new("local-path");
    let store = one_model(&server, &bytes, &root);

    assert_eq!(
        store.local_path(&id("m")).unwrap(),
        None,
        "before installing"
    );
    store.install(&id("m"), &ignore, &never()).unwrap_err();
    assert_eq!(store.local_path(&id("m")).unwrap(), None, "after a failure");
    store.install(&id("m"), &ignore, &never()).unwrap();
    let folder = store.local_path(&id("m")).unwrap().expect("installed");
    assert!(folder.is_dir());
    store.delete(&id("m")).unwrap();
    assert_eq!(store.local_path(&id("m")).unwrap(), None, "after deleting");
}

#[test]
fn a_model_whose_files_were_removed_from_disk_is_no_longer_installed() {
    let server = TestServer::start();
    let bytes = content(LEN, 67);
    server.route("/m.bin", Reply::file(&bytes));
    let root = TempDir::new("removed-by-hand");
    let store = one_model(&server, &bytes, &root);
    store.install(&id("m"), &ignore, &never()).unwrap();
    let folder = store.local_path(&id("m")).unwrap().unwrap();
    fs::remove_file(folder.join("m.bin")).unwrap();

    assert_eq!(store.local_path(&id("m")).unwrap(), None);
    assert!(
        !store
            .get_status(&id("m"))
            .unwrap()
            .unwrap()
            .state
            .is_installed()
    );
    assert!(store.mark_loaded(&id("m"), true).is_err());
}

#[test]
fn a_model_another_program_is_working_on_is_left_alone() {
    let server = TestServer::start();
    let bytes = content(LEN, 71);
    server.route("/m.bin", Reply::file(&bytes));
    let root = TempDir::new("locked-by-another-program");
    let store = one_model(&server, &bytes, &root);

    // Another program using the same folder holds the model's lock.
    let locks = root.path().join("locks");
    fs::create_dir_all(&locks).unwrap();
    let held = fs::File::create(locks.join("m.lock")).unwrap();
    held.lock().unwrap();

    let err = store.install(&id("m"), &ignore, &never()).unwrap_err();
    assert_eq!(err.kind, ErrorKind::AlreadyExists, "{err}");
    assert!(err.message.contains("another program"), "{err}");
    let err = store.delete(&id("m")).unwrap_err();
    assert_eq!(err.kind, ErrorKind::AlreadyExists, "{err}");
    assert!(
        server.requests_for("/m.bin").is_empty(),
        "nothing was fetched"
    );

    drop(held);
    store.install(&id("m"), &ignore, &never()).unwrap();
    assert!(store.delete(&id("m")).unwrap());
}

#[test]
fn a_model_another_program_has_loaded_is_not_deleted() {
    let server = TestServer::start();
    let bytes = content(LEN, 73);
    server.route("/m.bin", Reply::file(&bytes));
    let root = TempDir::new("in-use-by-another-program");
    let ours = one_model(&server, &bytes, &root);
    ours.install(&id("m"), &ignore, &never()).unwrap();

    // Another program working in the same folder loads the model.
    let theirs = one_model(&server, &bytes, &root);
    theirs.mark_loaded(&id("m"), true).unwrap();

    let err = ours.delete(&id("m")).unwrap_err();
    assert_eq!(err.kind, ErrorKind::AlreadyExists, "{err}");
    assert!(err.message.contains("another program"), "{err}");
    assert!(
        ours.local_path(&id("m")).unwrap().is_some(),
        "still installed"
    );

    // Two programs can have it loaded at once, and it goes only when neither has.
    ours.mark_loaded(&id("m"), true).unwrap();
    ours.mark_loaded(&id("m"), false).unwrap();
    assert!(ours.delete(&id("m")).is_err());
    theirs.mark_loaded(&id("m"), false).unwrap();
    assert!(ours.delete(&id("m")).unwrap());
}

#[test]
fn a_new_store_leaves_the_trash_of_a_model_another_program_is_replacing() {
    let root = TempDir::new("trash-in-use");
    let trash = root.path().join("trash");
    fs::create_dir_all(trash.join("m-1-2-3")).unwrap();
    fs::create_dir_all(trash.join("other-1-2-3")).unwrap();
    let locks = root.path().join("locks");
    fs::create_dir_all(&locks).unwrap();
    let held = fs::File::create(locks.join("m.lock")).unwrap();
    held.lock().unwrap();

    let server = TestServer::start();
    let _store = one_model(&server, &content(LEN, 74), &root);
    assert!(
        trash.join("m-1-2-3").exists(),
        "still needed for a rollback"
    );
    assert!(!trash.join("other-1-2-3").exists());
}
