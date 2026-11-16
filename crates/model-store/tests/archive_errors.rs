mod support;

use std::fs;
use std::path::Path;

use dictation_types::{AppError, ArchiveFormat, ErrorKind};
use model_store::archive::unpack;
use support::archive::{file, tar_bz2};
use support::{TempDir, never};

fn unpack_bytes(archive: &[u8], root: &TempDir, into: &Path) -> Result<(), AppError> {
    let packed = root.join("pack.tar.bz2");
    fs::write(&packed, archive).unwrap();
    unpack(&packed, ArchiveFormat::TarBz2, into, &never())
}

#[test]
fn something_in_the_way_of_a_folder_is_a_storage_error_not_a_damaged_archive() {
    let root = TempDir::new("unpack-in-the-way");
    let into = root.join("model");
    fs::create_dir_all(&into).unwrap();
    // A file where the archive needs a folder.
    fs::write(into.join("weights"), b"in the way").unwrap();
    let archive = tar_bz2(&[file("weights/encoder.onnx", b"encoder weights")]);

    let err = unpack_bytes(&archive, &root, &into).expect_err("the folder cannot be made");
    assert_eq!(err.kind, ErrorKind::Storage, "{err}");
    assert!(!err.message.contains("damaged"), "{err}");
    assert!(
        err.message.contains("os error"),
        "the system's words are kept: {err}"
    );
    assert_said_once(&err, "os error");
    assert!(err.message.contains("weights/encoder.onnx"), "{err}");

    // A folder that may not be written to is a storage problem too.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let locked = root.join("locked");
        fs::create_dir_all(&locked).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();
        let archive = tar_bz2(&[file("tokens.txt", b"a\nb\n")]);
        let result = unpack_bytes(&archive, &root, &locked);
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        let err = result.expect_err("nothing may be written there");
        assert_eq!(err.kind, ErrorKind::Storage, "{err}");
        assert!(!err.message.contains("damaged"), "{err}");
    }
}

/// `err` says `words` once, without repeating itself.
fn assert_said_once(err: &AppError, words: &str) {
    let text = err.message.to_lowercase();
    assert_eq!(text.matches(&words.to_lowercase()).count(), 1, "{err}");
    assert_eq!(text.matches("failed to").count(), 1, "{err}");
}
