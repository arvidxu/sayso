mod support;

use std::fs;

use dictation_api::ModelStore;
use dictation_types::ErrorKind;
use model_store::LocalModelStore;
use serde_json::json;
use support::archive::{RawEntry, file, folder, tar_bz2};
use support::server::{Reply, TestServer};
use support::{TempDir, catalog, file_entry, files_under, id, ignore, model_entry, never, store};
use tar::EntryType;

fn archive_store(server: &TestServer, archive: &[u8], root: &TempDir) -> LocalModelStore {
    server.route("/pack.tar.bz2", Reply::file(archive));
    let mut entry = file_entry("pack.tar.bz2", archive, &[server.url("/pack.tar.bz2")]);
    entry["unpack"] = json!("tar.bz2");
    store(
        catalog(vec![model_entry("packed", vec![entry])]),
        root.path(),
    )
}

fn assert_refused(server: &TestServer, archive: &[u8], root: &TempDir, what: &str) {
    let store = archive_store(server, archive, root);
    let result = store.install(&id("packed"), &ignore, &never());
    assert!(result.is_err(), "an archive with {what} was installed");
    let err = result.unwrap_err();
    assert_ne!(err.kind, ErrorKind::Cancelled, "{what}: {err}");
    assert!(
        !store
            .get_status(&id("packed"))
            .unwrap()
            .unwrap()
            .state
            .is_installed(),
        "{what}: the model counts as installed"
    );
    assert_eq!(store.local_path(&id("packed")).unwrap(), None, "{what}");
}

#[test]
fn an_archive_is_unpacked_into_the_model_folder_and_removed() {
    let server = TestServer::start();
    let archive = tar_bz2(&[
        folder("model/"),
        file("model/encoder.onnx", b"encoder weights"),
        file("model/tokens.txt", b"a\nb\nc\n"),
        file("README", b"read me"),
    ]);
    let root = TempDir::new("archive");
    let store = archive_store(&server, &archive, &root);

    let status = store
        .install(&id("packed"), &ignore, &never())
        .expect("installed");
    assert!(status.state.is_installed());
    let dir = store.local_path(&id("packed")).unwrap().expect("a folder");
    assert_eq!(
        fs::read(dir.join("model/encoder.onnx")).unwrap(),
        b"encoder weights"
    );
    assert_eq!(
        fs::read(dir.join("model/tokens.txt")).unwrap(),
        b"a\nb\nc\n"
    );
    assert_eq!(fs::read(dir.join("README")).unwrap(), b"read me");
    assert!(
        !dir.join("pack.tar.bz2").exists(),
        "the archive is not kept"
    );
    assert!(
        files_under(&dir).iter().all(|(_, data)| data != &archive),
        "no copy of the archive is kept"
    );

    // Still installed afterwards, without fetching it again.
    let asked = server.requests().len();
    store.install(&id("packed"), &ignore, &never()).unwrap();
    assert_eq!(server.requests().len(), asked);
}

#[test]
fn an_archive_entry_with_parent_folder_steps_is_refused() {
    let server = TestServer::start();
    let root = TempDir::new("archive-dotdot");
    let marker = format!("escaped-{}", std::process::id());
    let name = format!("model/../../../{marker}.txt");
    let archive = tar_bz2(&[file("model/ok.txt", b"fine"), file(&name, b"escaped")]);
    assert_refused(&server, &archive, &root, "a '..' entry");
    let escaped: Vec<String> = files_under(root.path())
        .into_iter()
        .map(|(path, _)| path)
        .filter(|path| path.contains(&marker))
        .collect();
    assert!(escaped.is_empty(), "files escaped: {escaped:?}");
}

#[test]
fn an_archive_entry_with_an_absolute_path_is_refused() {
    let server = TestServer::start();
    let root = TempDir::new("archive-absolute");
    let outside = TempDir::new("archive-absolute-target");
    let target = outside.join("absolute.txt");
    let name = target.to_string_lossy().replace('\\', "/");
    let archive = tar_bz2(&[file("ok.txt", b"fine"), file(&name, b"absolute")]);
    assert_refused(&server, &archive, &root, "an absolute entry");
    assert!(!target.exists(), "the absolute path was written");
}

#[test]
fn an_archive_with_a_symbolic_link_is_refused() {
    let server = TestServer::start();
    let root = TempDir::new("archive-symlink");
    let link = RawEntry {
        name: "model/link",
        kind: EntryType::Symlink,
        link: "../../outside",
        data: b"",
    };
    let archive = tar_bz2(&[file("model/ok.txt", b"fine"), link]);
    assert_refused(&server, &archive, &root, "a symbolic link");
}

#[test]
fn an_archive_with_a_hard_link_is_refused() {
    let server = TestServer::start();
    let root = TempDir::new("archive-hardlink");
    let link = RawEntry {
        name: "model/hard",
        kind: EntryType::Link,
        link: "model/ok.txt",
        data: b"",
    };
    let archive = tar_bz2(&[file("model/ok.txt", b"fine"), link]);
    assert_refused(&server, &archive, &root, "a hard link");
}

#[test]
fn a_damaged_archive_is_refused() {
    let server = TestServer::start();
    let root = TempDir::new("archive-damaged");
    assert_refused(
        &server,
        b"this is not a compressed archive at all",
        &root,
        "damaged data",
    );
}

#[test]
fn a_read_only_folder_in_an_archive_is_installed_and_deleted() {
    use bzip2::{Compression, write::BzEncoder};
    let mut builder = tar::Builder::new(BzEncoder::new(Vec::new(), Compression::fast()));
    for (kind, mode, path, data) in [
        (EntryType::Directory, 0o555, "ro/", &b""[..]),
        (EntryType::Regular, 0o444, "ro/a.bin", b"weights"),
    ] {
        let mut header = tar::Header::new_ustar();
        header.set_entry_type(kind);
        header.set_mode(mode);
        header.set_size(data.len() as u64);
        builder.append_data(&mut header, path, data).unwrap();
    }
    let archive = builder.into_inner().unwrap().finish().unwrap();
    let server = TestServer::start();
    let root = TempDir::new("archive-read-only");
    let store = archive_store(&server, &archive, &root);
    store.install(&id("packed"), &ignore, &never()).unwrap();
    let folder = store.local_path(&id("packed")).unwrap().unwrap();
    assert!(folder.join("ro/a.bin").is_file());

    assert!(store.delete(&id("packed")).unwrap());
    assert!(!folder.exists(), "the model's files stayed");
}
