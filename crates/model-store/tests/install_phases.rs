mod support;

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use dictation_api::ModelStore;
use dictation_types::{DownloadProgress, InstallPhase, ModelInstallState};
use serde_json::json;
use support::archive::{file, tar_bz2};
use support::server::{Reply, TestServer};
use support::{TempDir, catalog, content, file_entry, id, ignore, model_entry, never, store};

fn record(events: &RefCell<Vec<DownloadProgress>>) -> impl Fn(&DownloadProgress) + '_ {
    |event| events.borrow_mut().push(event.clone())
}

/// The phases in the order they were first reported for `path`.
fn phases_of(events: &[DownloadProgress], path: &str) -> Vec<InstallPhase> {
    let mut phases = Vec::new();
    for event in events.iter().filter(|e| e.file_path == path) {
        if phases.last() != Some(&event.phase) {
            phases.push(event.phase);
        }
    }
    phases
}

#[test]
fn each_file_is_downloaded_and_then_verified() {
    let server = TestServer::start();
    let first = content(6_000, 121);
    let second = content(4_000, 122);
    server.route("/first.bin", Reply::file(&first));
    server.route("/second.bin", Reply::file(&second));
    let root = TempDir::new("phases");
    let store = store(
        catalog(vec![model_entry(
            "m",
            vec![
                file_entry("first.bin", &first, &[server.url("/first.bin")]),
                file_entry("second.bin", &second, &[server.url("/second.bin")]),
            ],
        )]),
        root.path(),
    );
    let events = RefCell::new(Vec::new());
    store.install(&id("m"), &record(&events), &never()).unwrap();
    let events = events.into_inner();

    for path in ["first.bin", "second.bin"] {
        let phases = phases_of(&events, path);
        let downloading = phases.iter().position(|p| *p == InstallPhase::Downloading);
        let verifying = phases.iter().rposition(|p| *p == InstallPhase::Verifying);
        assert!(
            downloading.is_some(),
            "{path}: no download phase in {phases:?}"
        );
        assert!(verifying.is_some(), "{path}: no verify phase in {phases:?}");
        assert!(
            downloading < verifying,
            "{path}: verified before downloading"
        );
        assert!(!phases.contains(&InstallPhase::Unpacking), "{path}");
    }
    let total = (first.len() + second.len()) as u64;
    let mut previous = 0;
    for event in events
        .iter()
        .filter(|e| e.phase == InstallPhase::Downloading)
    {
        assert!(
            event.downloaded_bytes >= previous,
            "download progress went back"
        );
        assert!(event.downloaded_bytes <= total);
        previous = event.downloaded_bytes;
    }
    assert!(events.iter().all(|e| e.total_bytes == total));
}

#[test]
fn an_archive_is_unpacked_after_it_was_verified() {
    let server = TestServer::start();
    let archive = tar_bz2(&[file("model/weights.bin", &content(5_000, 123))]);
    server.route("/pack.tar.bz2", Reply::file(&archive));
    let mut entry = file_entry("pack.tar.bz2", &archive, &[server.url("/pack.tar.bz2")]);
    entry["unpack"] = json!("tar.bz2");
    let root = TempDir::new("phases-archive");
    let store = store(catalog(vec![model_entry("m", vec![entry])]), root.path());
    let events = RefCell::new(Vec::new());
    store.install(&id("m"), &record(&events), &never()).unwrap();
    let events = events.into_inner();

    let phases = phases_of(&events, "pack.tar.bz2");
    let verifying = phases.iter().position(|p| *p == InstallPhase::Verifying);
    let unpacking = phases.iter().rposition(|p| *p == InstallPhase::Unpacking);
    assert!(unpacking.is_some(), "no unpack phase in {phases:?}");
    assert!(
        verifying < unpacking,
        "unpacked before verifying: {phases:?}"
    );
}

#[test]
fn a_file_found_in_the_cache_is_still_verified() {
    let server = TestServer::start();
    let bytes = content(3_000, 124);
    server.route("/m.bin", Reply::file(&bytes));
    let root = TempDir::new("phases-cached");
    let cache = TempDir::new("phases-cache");
    let blobs = cache.join("models--a--b").join("blobs");
    std::fs::create_dir_all(&blobs).unwrap();
    std::fs::write(blobs.join(support::sha256_hex(&bytes)), &bytes).unwrap();
    let store = support::store_with(
        catalog(vec![model_entry(
            "m",
            vec![file_entry("m.bin", &bytes, &[server.url("/m.bin")])],
        )]),
        root.path(),
        vec![cache.path().to_path_buf()],
    );
    let events = RefCell::new(Vec::new());
    store.install(&id("m"), &record(&events), &never()).unwrap();
    assert!(server.requests().is_empty());
    let phases = phases_of(&events.into_inner(), "m.bin");
    assert!(phases.contains(&InstallPhase::Verifying), "{phases:?}");
}

#[test]
fn installing_a_model_that_is_already_installed_never_reports_downloading() {
    let server = TestServer::start();
    let bytes = content(2_000, 124);
    server.route("/m.bin", Reply::file(&bytes));
    let root = TempDir::new("phases-installed");
    let store = store(
        catalog(vec![model_entry(
            "m",
            vec![file_entry("m.bin", &bytes, &[server.url("/m.bin")])],
        )]),
        root.path(),
    );
    store.install(&id("m"), &ignore, &never()).unwrap();

    let done = AtomicBool::new(false);
    let downloading_seen = thread::scope(|scope| {
        let watcher = scope.spawn(|| {
            let mut seen = false;
            while !done.load(Ordering::SeqCst) {
                let status = store.get_status(&id("m")).unwrap().unwrap();
                seen |= matches!(status.state, ModelInstallState::Downloading { .. });
            }
            seen
        });
        for _ in 0..2_000 {
            let status = store.install(&id("m"), &ignore, &never()).unwrap();
            assert!(status.state.is_installed());
        }
        done.store(true, Ordering::SeqCst);
        watcher.join().unwrap()
    });
    assert!(
        !downloading_seen,
        "an installed model was reported as downloading"
    );
    assert_eq!(server.requests_for("/m.bin").len(), 1);
}
