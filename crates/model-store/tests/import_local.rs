mod support;

use std::fs;

use dictation_api::ModelStore;
use dictation_types::{ErrorKind, ModelInstallState};
use model_store::LocalModelStore;
use support::server::{Reply, TestServer};
use support::{
    TempDir, catalog, content, file_entry, files_under, id, ignore, model_entry, never, store,
};

fn import_store(root: &TempDir) -> LocalModelStore {
    let urls = ["https://example.invalid/unused".to_owned()];
    store(
        catalog(vec![model_entry(
            "m",
            vec![
                file_entry("a.bin", &content(2_000, 51), &urls),
                file_entry("sub/b.bin", &content(1_000, 52), &urls),
            ],
        )]),
        root.path(),
    )
}

fn good_source() -> TempDir {
    let source = TempDir::new("import-source");
    fs::write(source.join("a.bin"), content(2_000, 51)).unwrap();
    fs::create_dir_all(source.join("sub")).unwrap();
    fs::write(source.join("sub/b.bin"), content(1_000, 52)).unwrap();
    fs::write(source.join("notes.txt"), b"not part of the model").unwrap();
    source
}

fn state(store: &LocalModelStore) -> ModelInstallState {
    store.get_status(&id("m")).unwrap().unwrap().state
}

#[test]
fn good_files_are_installed_and_the_source_folder_is_left_unchanged() {
    let root = TempDir::new("import");
    let store = import_store(&root);
    let source = good_source();
    let before = files_under(source.path());

    let status = store
        .import_local(&id("m"), source.path())
        .expect("imported");
    assert!(status.state.is_installed());
    assert!(state(&store).is_installed());
    let folder = store.local_path(&id("m")).unwrap().expect("a folder");
    assert_eq!(fs::read(folder.join("a.bin")).unwrap(), content(2_000, 51));
    assert_eq!(
        fs::read(folder.join("sub/b.bin")).unwrap(),
        content(1_000, 52)
    );
    assert_eq!(files_under(source.path()), before, "the source changed");
    assert!(!folder.starts_with(source.path()));
}

#[test]
fn a_mismatching_file_fails_with_a_checksum_mismatch_and_marks_the_model_corrupted() {
    let root = TempDir::new("import-bad");
    let store = import_store(&root);
    let source = good_source();
    fs::write(source.join("sub/b.bin"), content(1_000, 99)).unwrap();
    let before = files_under(source.path());

    let err = store.import_local(&id("m"), source.path()).unwrap_err();
    assert_eq!(err.kind, ErrorKind::ChecksumMismatch, "{err}");
    assert!(
        matches!(state(&store), ModelInstallState::Corrupted { .. }),
        "{:?}",
        state(&store)
    );
    assert_eq!(store.local_path(&id("m")).unwrap(), None);
    assert_eq!(files_under(source.path()), before, "the source changed");
}

#[test]
fn a_file_of_the_wrong_size_counts_as_a_mismatch() {
    let root = TempDir::new("import-size");
    let store = import_store(&root);
    let source = good_source();
    fs::write(source.join("a.bin"), content(1_999, 51)).unwrap();
    let err = store.import_local(&id("m"), source.path()).unwrap_err();
    assert_eq!(err.kind, ErrorKind::ChecksumMismatch, "{err}");
    assert!(matches!(state(&store), ModelInstallState::Corrupted { .. }));
}

#[test]
fn a_missing_file_fails_as_not_found_without_marking_the_model_corrupted() {
    let root = TempDir::new("import-missing");
    let store = import_store(&root);
    let source = good_source();
    fs::remove_file(source.join("sub/b.bin")).unwrap();

    let err = store.import_local(&id("m"), source.path()).unwrap_err();
    assert_eq!(err.kind, ErrorKind::NotFound, "{err}");
    assert_eq!(state(&store), ModelInstallState::NotInstalled);
    assert_eq!(store.local_path(&id("m")).unwrap(), None);
}

#[test]
fn importing_from_a_folder_that_does_not_exist_fails_as_not_found() {
    let root = TempDir::new("import-nowhere");
    let store = import_store(&root);
    let missing = root.join("no-such-folder");
    let err = store.import_local(&id("m"), &missing).unwrap_err();
    assert_eq!(err.kind, ErrorKind::NotFound, "{err}");
    assert_eq!(state(&store), ModelInstallState::NotInstalled);
}

#[test]
fn a_later_good_import_clears_the_corrupted_mark_and_installs() {
    let root = TempDir::new("import-recover");
    let store = import_store(&root);
    let bad = good_source();
    fs::write(bad.join("a.bin"), content(2_000, 98)).unwrap();
    store.import_local(&id("m"), bad.path()).unwrap_err();
    assert!(matches!(state(&store), ModelInstallState::Corrupted { .. }));

    let good = good_source();
    store.import_local(&id("m"), good.path()).expect("imported");
    assert!(state(&store).is_installed(), "{:?}", state(&store));
    assert!(store.local_path(&id("m")).unwrap().is_some());
}

#[test]
fn a_failed_import_leaves_an_installed_model_installed() {
    let root = TempDir::new("import-keep");
    let store = import_store(&root);
    store.import_local(&id("m"), good_source().path()).unwrap();
    let missing = good_source();
    fs::remove_file(missing.join("a.bin")).unwrap();
    store.import_local(&id("m"), missing.path()).unwrap_err();
    assert!(state(&store).is_installed());
    let folder = store.local_path(&id("m")).unwrap().unwrap();
    assert_eq!(fs::read(folder.join("a.bin")).unwrap(), content(2_000, 51));
}

#[test]
fn a_mismatching_import_leaves_an_installed_model_installed_and_not_corrupted() {
    let root = TempDir::new("import-keep-bad");
    let store = import_store(&root);
    store.import_local(&id("m"), good_source().path()).unwrap();
    let bad = good_source();
    fs::write(bad.join("a.bin"), content(2_000, 97)).unwrap();

    let err = store.import_local(&id("m"), bad.path()).unwrap_err();
    assert_eq!(err.kind, ErrorKind::ChecksumMismatch, "{err}");
    assert!(state(&store).is_installed(), "{:?}", state(&store));
    let folder = store
        .local_path(&id("m"))
        .unwrap()
        .expect("still installed");
    assert_eq!(fs::read(folder.join("a.bin")).unwrap(), content(2_000, 51));
}

#[test]
fn a_later_successful_download_clears_the_corrupted_mark() {
    let server = TestServer::start();
    let bytes = content(1_500, 53);
    server.route("/m.bin", Reply::file(&bytes));
    let root = TempDir::new("import-then-download");
    let store = store(
        catalog(vec![model_entry(
            "m",
            vec![file_entry("m.bin", &bytes, &[server.url("/m.bin")])],
        )]),
        root.path(),
    );
    let bad = TempDir::new("import-then-download-source");
    fs::write(bad.join("m.bin"), content(1_500, 54)).unwrap();
    store.import_local(&id("m"), bad.path()).unwrap_err();
    assert!(matches!(state(&store), ModelInstallState::Corrupted { .. }));

    store
        .install(&id("m"), &ignore, &never())
        .expect("installed");
    assert!(state(&store).is_installed(), "{:?}", state(&store));
}
