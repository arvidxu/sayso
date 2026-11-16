mod support;

use dictation_api::ModelStore;
use dictation_types::ErrorKind;
use serde_json::json;
use support::archive::{file, tar_bz2};
use support::server::{Reply, TestServer};
use support::{
    TempDir, catalog, file_entry, id, ignore, model_entry, never, store_tuned, test_overrides,
    tuning_with,
};

fn install_with_ratio(ratio: f64) -> (TestServer, Result<(), dictation_types::AppError>) {
    let server = TestServer::start();
    let archive = tar_bz2(&[file("model/weights.bin", &[3u8; 2_048])]);
    server.route("/pack.tar.bz2", Reply::file(&archive));
    let mut entry = file_entry("pack.tar.bz2", &archive, &[server.url("/pack.tar.bz2")]);
    entry["unpack"] = json!("tar.bz2");
    let mut overrides = test_overrides(1024);
    overrides["unpackedSizeRatio"] = json!(ratio);
    let root = TempDir::new("archive-room");
    let store = store_tuned(
        catalog(vec![model_entry("packed", vec![entry])]),
        root.path(),
        &tuning_with(overrides),
    );
    let result = store.install(&id("packed"), &ignore, &never()).map(|_| ());
    (server, result)
}

#[test]
fn an_archive_whose_content_would_not_fit_is_not_downloaded() {
    // Far more than any disk holds, for an archive of a few kilobytes.
    let (server, result) = install_with_ratio(1e15);
    let err = result.expect_err("there is no room for what it unpacks to");
    assert_eq!(err.kind, ErrorKind::Storage, "{err}");
    assert!(err.message.contains("not enough room"), "{err}");
    assert!(server.requests().is_empty(), "it was downloaded anyway");
}
