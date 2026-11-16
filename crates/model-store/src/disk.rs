use std::collections::BTreeSet;
use std::fmt::Display;
use std::fs::{self, File};
use std::io::{ErrorKind as IoErrorKind, Read, Write};
use std::path::{Path, PathBuf};

use dictation_types::{AppError, AppResult, CancelToken};

pub(crate) fn storage(path: &Path, what: &str, err: impl Display) -> AppError {
    AppError::storage(format!("failed to {what} '{}': {err}", path.display()))
}

/// A file that is not there is fine.
pub(crate) fn remove_file(path: &Path) -> AppResult<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == IoErrorKind::NotFound => Ok(()),
        Err(err) => Err(storage(path, "remove", err)),
    }
}

/// A folder that is not there is fine.
pub(crate) fn remove_dir(path: &Path) -> AppResult<()> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == IoErrorKind::NotFound => Ok(()),
        Err(err) => Err(storage(path, "remove", err)),
    }
}

pub(crate) fn create_dir(path: &Path) -> AppResult<()> {
    fs::create_dir_all(path).map_err(|err| storage(path, "create", err))
}

pub(crate) fn create_parent(path: &Path) -> AppResult<()> {
    path.parent().map_or(Ok(()), create_dir)
}

/// A disk that will not say how much is free is not refused; the write then reports a full disk.
pub(crate) fn ensure_room(dir: &Path, needed: u64) -> AppResult<()> {
    match fs4::available_space(dir) {
        Ok(free) => check_room(dir, needed, free),
        Err(_) => Ok(()),
    }
}

pub(crate) fn check_room(dir: &Path, needed: u64, free: u64) -> AppResult<()> {
    if free >= needed {
        return Ok(());
    }
    Err(AppError::storage(format!(
        "not enough room on the disk holding '{}': {needed} bytes are needed, {free} are free",
        dir.display()
    )))
}

/// 0 when there is no file.
pub(crate) fn len_of(path: &Path) -> u64 {
    fs::metadata(path).map_or(0, |meta| meta.len())
}

/// The copy has reached the disk when this returns; on failure or cancel it is removed.
pub(crate) fn copy_file(
    from: &Path,
    to: &Path,
    buffer_bytes: usize,
    on_bytes: &mut dyn FnMut(u64),
    cancel: &CancelToken,
) -> AppResult<()> {
    let copied = copy_into(from, to, buffer_bytes, on_bytes, cancel);
    if copied.is_err() {
        let _ = fs::remove_file(to);
    }
    copied
}

fn copy_into(
    from: &Path,
    to: &Path,
    buffer_bytes: usize,
    on_bytes: &mut dyn FnMut(u64),
    cancel: &CancelToken,
) -> AppResult<()> {
    let mut input = File::open(from).map_err(|err| storage(from, "open", err))?;
    let mut output = File::create(to).map_err(|err| storage(to, "create", err))?;
    let mut buffer = vec![0u8; buffer_bytes.max(1)];
    let mut copied = 0u64;
    loop {
        if cancel.is_cancelled() {
            return Err(AppError::cancelled("copying the model file was cancelled"));
        }
        let read = input
            .read(&mut buffer)
            .map_err(|err| storage(from, "read", err))?;
        if read == 0 {
            break;
        }
        output
            .write_all(&buffer[..read])
            .map_err(|err| storage(to, "write", err))?;
        copied += read as u64;
        on_bytes(copied);
    }
    output.flush().map_err(|err| storage(to, "write", err))?;
    output.sync_all().map_err(|err| storage(to, "save", err))
}

/// Hard-links when the system allows it, otherwise copies.
pub(crate) fn link_or_copy(from: &Path, to: &Path, buffer_bytes: usize) -> AppResult<()> {
    if fs::hard_link(from, to).is_ok() {
        return Ok(());
    }
    copy_file(from, to, buffer_bytes, &mut |_| {}, &CancelToken::new())
}

/// Paths are relative to `dir` with `/` between folders.
pub(crate) fn list_files(dir: &Path) -> AppResult<Vec<(String, u64)>> {
    let mut found = Vec::new();
    let mut pending: Vec<PathBuf> = vec![dir.to_path_buf()];
    while let Some(current) = pending.pop() {
        let entries = fs::read_dir(&current).map_err(|err| storage(&current, "list", err))?;
        for entry in entries {
            let entry = entry.map_err(|err| storage(&current, "list", err))?;
            let path = entry.path();
            let meta = entry
                .metadata()
                .map_err(|err| storage(&path, "inspect", err))?;
            if meta.is_dir() {
                pending.push(path);
            } else if meta.is_file() {
                let relative = path
                    .strip_prefix(dir)
                    .map_err(|err| storage(&path, "locate", err))?
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                found.push((relative, meta.len()));
            }
        }
    }
    found.sort();
    Ok(found)
}

/// Unpacked read-only entries stay read-only on this system, and a file can only be saved to
/// disk through a writable handle, so the mark is removed.
#[cfg(windows)]
#[allow(
    clippy::permissions_set_readonly_false,
    reason = "on Windows this only takes off the read-only mark; it gives nobody else access"
)]
pub(crate) fn make_writable(path: &Path) -> std::io::Result<()> {
    let mut permissions = fs::metadata(path)?.permissions();
    if !permissions.readonly() {
        return Ok(());
    }
    permissions.set_readonly(false);
    fs::set_permissions(path, permissions)
}

#[cfg(not(windows))]
pub(crate) fn make_writable(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

pub(crate) fn sync_tree(dir: &Path) -> AppResult<()> {
    let mut folders: BTreeSet<PathBuf> = BTreeSet::new();
    for (relative, _) in list_files(dir)? {
        let path = dir.join(relative);
        // Some systems save a file to disk only through a handle that may write to it.
        fs::OpenOptions::new()
            .read(true)
            .write(cfg!(windows))
            .open(&path)
            .and_then(|file| file.sync_all())
            .map_err(|err| storage(&path, "save", err))?;
        // One folder already known means its ancestors are known too.
        for folder in path.ancestors().skip(1).take_while(|folder| *folder != dir) {
            if !folders.insert(folder.to_path_buf()) {
                break;
            }
        }
    }
    folders.insert(dir.to_path_buf());
    folders.iter().try_for_each(|folder| sync_dir(folder))
}

/// Only some systems can save folder entries to disk; elsewhere this does nothing.
pub(crate) fn sync_dir(dir: &Path) -> AppResult<()> {
    if cfg!(unix) {
        File::open(dir)
            .and_then(|folder| folder.sync_all())
            .map_err(|err| storage(dir, "save", err))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use dictation_types::ErrorKind;

    use super::check_room;

    #[test]
    fn room_is_enough_only_when_as_much_is_free_as_is_needed() {
        let dir = Path::new("some/folder");
        assert!(check_room(dir, 0, 0).is_ok());
        assert!(check_room(dir, 1, 1).is_ok());
        assert!(check_room(dir, 1, 2).is_ok());
        let err = check_room(dir, 2, 1).expect_err("one byte short");
        assert_eq!(err.kind, ErrorKind::Storage);
        assert!(
            err.message.contains("2 bytes are needed, 1 are free"),
            "{err}"
        );
        assert!(err.message.contains("some/folder"), "{err}");
    }
}
