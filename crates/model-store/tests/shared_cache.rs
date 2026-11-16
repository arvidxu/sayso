mod support;

use std::fs;
use std::path::Path;

use dictation_api::ModelStore;
use model_store::LocalModelStore;
use model_store::local_cache::find_by_checksum;
use support::server::{Reply, TestServer};
use support::{
    TempDir, caches, catalog, content, file_entry, id, ignore, model_entry, never, sha256_hex,
    store_with,
};

/// Puts `data` in `cache` as the blob named `name` of the repository `models--a--b`.
fn put_blob(cache: &Path, name: &str, data: &[u8]) {
    let blobs = cache.join("models--a--b").join("blobs");
    fs::create_dir_all(&blobs).unwrap();
    fs::write(blobs.join(name), data).unwrap();
}

fn cached_store(
    server: &TestServer,
    bytes: &[u8],
    root: &TempDir,
    cache: &Path,
) -> LocalModelStore {
    server.route("/m.bin", Reply::file(bytes));
    store_with(
        catalog(vec![model_entry(
            "m",
            vec![file_entry("weights/m.bin", bytes, &[server.url("/m.bin")])],
        )]),
        root.path(),
        vec![cache.to_path_buf()],
    )
}

#[test]
fn a_file_in_the_cache_is_used_without_asking_the_server() {
    let server = TestServer::start();
    let bytes = content(6_000, 41);
    let root = TempDir::new("cache-root");
    let cache = TempDir::new("cache-cache");
    put_blob(cache.path(), &sha256_hex(&bytes), &bytes);
    let store = cached_store(&server, &bytes, &root, cache.path());

    store
        .install(&id("m"), &ignore, &never())
        .expect("installed from the cache");
    assert!(server.requests().is_empty(), "the server was asked");
    let folder = store.local_path(&id("m")).unwrap().expect("installed");
    assert_eq!(fs::read(folder.join("weights/m.bin")).unwrap(), bytes);
    let blob = cache
        .join("models--a--b")
        .join("blobs")
        .join(sha256_hex(&bytes));
    assert_eq!(
        fs::read(blob).unwrap(),
        bytes,
        "the cache is left as it was"
    );
}

#[test]
fn a_cached_file_named_by_the_checksum_but_with_other_content_is_downloaded_instead() {
    let server = TestServer::start();
    let bytes = content(6_000, 42);
    let root = TempDir::new("cache-wrong-root");
    let cache = TempDir::new("cache-wrong-cache");
    put_blob(cache.path(), &sha256_hex(&bytes), &content(6_000, 43));
    let store = cached_store(&server, &bytes, &root, cache.path());

    store
        .install(&id("m"), &ignore, &never())
        .expect("installed from the server");
    let requests = server.requests_for("/m.bin");
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].range, None,
        "the wrong content was not continued"
    );
    let folder = store.local_path(&id("m")).unwrap().expect("installed");
    assert_eq!(fs::read(folder.join("weights/m.bin")).unwrap(), bytes);
}

#[test]
fn a_cached_file_that_is_too_short_is_downloaded_instead() {
    let server = TestServer::start();
    let bytes = content(6_000, 44);
    let root = TempDir::new("cache-short-root");
    let cache = TempDir::new("cache-short-cache");
    put_blob(cache.path(), &sha256_hex(&bytes), &bytes[..1_000]);
    let store = cached_store(&server, &bytes, &root, cache.path());

    store
        .install(&id("m"), &ignore, &never())
        .expect("installed");
    let requests = server.requests_for("/m.bin");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].range, None);
    let folder = store.local_path(&id("m")).unwrap().unwrap();
    assert_eq!(fs::read(folder.join("weights/m.bin")).unwrap(), bytes);
}

#[test]
fn a_file_is_found_by_its_checksum_in_any_repository_of_any_cache_folder() {
    let empty = TempDir::new("cache-empty");
    let cache = TempDir::new("cache-find");
    let sha = sha256_hex(b"data");
    let blobs = cache.join("models--x--y").join("blobs");
    fs::create_dir_all(&blobs).unwrap();
    fs::write(blobs.join(&sha), b"data").unwrap();

    let dirs = caches(&[empty.path().to_path_buf(), cache.path().to_path_buf()]);
    assert_eq!(find_by_checksum(&dirs, &sha), Some(blobs.join(&sha)));
    assert_eq!(
        find_by_checksum(&dirs, &sha.to_ascii_uppercase()),
        Some(blobs.join(&sha))
    );
    assert_eq!(find_by_checksum(&dirs, &sha256_hex(b"other")), None);
    assert_eq!(find_by_checksum(&dirs, "../../etc"), None);
    assert_eq!(find_by_checksum(&dirs, ""), None);
}

#[test]
fn where_caches_are_and_how_files_are_named_comes_from_the_given_locations() {
    use model_store::{LocalCacheLocation, resolve_local_caches};

    let base = TempDir::new("cache-locations");
    let sha = sha256_hex(b"data");
    let folder = base.join("store").join("by-checksum");
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join(format!("{sha}.bin")), b"data").unwrap();

    // The variable name is made up for this test, so nothing else sets it.
    let variable = format!("MODELS_TEST_CACHE_{}", std::process::id());
    // SAFETY: set once before any read; other test threads reading meanwhile is accepted.
    unsafe { std::env::set_var(&variable, base.path()) };
    let location = |env_var: &str, sub_path: &str| LocalCacheLocation {
        env_var: Some(env_var.to_owned()),
        sub_path: sub_path.to_owned(),
        file_pattern: "by-checksum/{sha256}.bin".to_owned(),
    };
    let resolved = resolve_local_caches(&[
        location("MODELS_TEST_CACHE_THAT_IS_NOT_SET", "store"),
        location(&variable, "missing-folder"),
        location(&variable, "store"),
    ]);
    assert_eq!(resolved.len(), 1, "only the location that exists applies");
    assert_eq!(
        find_by_checksum(&resolved, &sha),
        Some(folder.join(format!("{sha}.bin")))
    );
}

#[cfg(unix)]
#[test]
fn a_cached_file_that_cannot_be_read_is_downloaded_instead() {
    use std::os::unix::fs::PermissionsExt;

    let server = TestServer::start();
    let bytes = content(6_000, 45);
    let root = TempDir::new("cache-unreadable-root");
    let cache = TempDir::new("cache-unreadable-cache");
    put_blob(cache.path(), &sha256_hex(&bytes), &bytes);
    let blob = cache.join("models--a--b/blobs").join(sha256_hex(&bytes));
    fs::set_permissions(&blob, fs::Permissions::from_mode(0o000)).unwrap();
    let store = cached_store(&server, &bytes, &root, cache.path());

    store
        .install(&id("m"), &ignore, &never())
        .expect("installed from the server");
    assert_eq!(server.requests_for("/m.bin").len(), 1);
}
