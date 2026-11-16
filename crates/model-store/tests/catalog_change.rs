mod support;

use std::fs;

use dictation_api::ModelStore;
use dictation_types::ModelInstallState;
use support::server::{Reply, TestServer};
use support::{TempDir, catalog, content, file_entry, id, ignore, model_entry, never, store};

#[test]
fn a_model_installed_from_an_older_catalog_entry_is_not_installed() {
    let server = TestServer::start();
    let old = content(3_000, 71);
    let new = content(3_000, 72);
    server.route("/old.bin", Reply::file(&old));
    server.route("/new.bin", Reply::file(&new));
    let root = TempDir::new("catalog-change");

    let before = store(
        catalog(vec![model_entry(
            "m",
            vec![file_entry("m.bin", &old, &[server.url("/old.bin")])],
        )]),
        root.path(),
    );
    before.install(&id("m"), &ignore, &never()).unwrap();
    assert!(before.local_path(&id("m")).unwrap().is_some());

    let after = store(
        catalog(vec![model_entry(
            "m",
            vec![file_entry("m.bin", &new, &[server.url("/new.bin")])],
        )]),
        root.path(),
    );
    assert_eq!(
        after.get_status(&id("m")).unwrap().unwrap().state,
        ModelInstallState::NotInstalled
    );
    assert_eq!(after.local_path(&id("m")).unwrap(), None);
    assert!(after.mark_loaded(&id("m"), true).is_err());

    // Installing again fetches the new file.
    after.install(&id("m"), &ignore, &never()).unwrap();
    assert_eq!(server.requests_for("/new.bin").len(), 1);
    let folder = after.local_path(&id("m")).unwrap().unwrap();
    assert_eq!(fs::read(folder.join("m.bin")).unwrap(), new);
}

#[test]
fn a_model_whose_catalog_entry_gained_a_file_is_not_installed() {
    let server = TestServer::start();
    let first = content(1_000, 73);
    let extra = content(1_000, 74);
    server.route("/first.bin", Reply::file(&first));
    let root = TempDir::new("catalog-grow");
    store(
        catalog(vec![model_entry(
            "m",
            vec![file_entry("first.bin", &first, &[server.url("/first.bin")])],
        )]),
        root.path(),
    )
    .install(&id("m"), &ignore, &never())
    .unwrap();

    let after = store(
        catalog(vec![model_entry(
            "m",
            vec![
                file_entry("first.bin", &first, &[server.url("/first.bin")]),
                file_entry("extra.bin", &extra, &[server.url("/extra.bin")]),
            ],
        )]),
        root.path(),
    );
    assert_eq!(after.local_path(&id("m")).unwrap(), None);
}

#[test]
fn an_unchanged_catalog_entry_keeps_the_model_installed() {
    let server = TestServer::start();
    let bytes = content(1_000, 75);
    server.route("/m.bin", Reply::file(&bytes));
    let root = TempDir::new("catalog-same");
    let models = || {
        catalog(vec![model_entry(
            "m",
            vec![file_entry("m.bin", &bytes, &[server.url("/m.bin")])],
        )])
    };
    store(models(), root.path())
        .install(&id("m"), &ignore, &never())
        .unwrap();
    assert!(
        store(models(), root.path())
            .local_path(&id("m"))
            .unwrap()
            .is_some()
    );
}
