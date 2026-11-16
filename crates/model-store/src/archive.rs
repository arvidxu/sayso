use std::cell::Cell;
use std::error::Error;
use std::fmt::Display;
use std::fs::{self, File};
use std::io::{self, BufReader, Read};
use std::path::{Component, Path};

use bzip2::read::BzDecoder;
use dictation_types::{AppError, AppResult, ArchiveFormat, CancelToken};
use tar::{Archive, EntryType};

use crate::disk::{create_dir, make_writable, storage};

/// Links, devices, and entries that leave `into` or replace an existing file make the whole
/// archive count as damaged; failures the system reports while writing are storage errors.
pub fn unpack(
    archive: &Path,
    format: ArchiveFormat,
    into: &Path,
    cancel: &CancelToken,
) -> AppResult<()> {
    let file = File::open(archive).map_err(|err| storage(archive, "open", err))?;
    create_dir(into)?;
    let read_failed = Cell::new(false);
    let result = match format {
        ArchiveFormat::TarBz2 => unpack_entries(
            Archive::new(Watched {
                inner: BzDecoder::new(BufReader::new(file)),
                cancel,
                read_failed: &read_failed,
            }),
            into,
            &read_failed,
        ),
    };
    if cancel.is_cancelled() {
        return Err(AppError::cancelled("unpacking the model was cancelled"));
    }
    result.map_err(|failure| failure.into_error(archive, into, read_failed.get()))
}

/// Checks `cancel` on every read, so one large entry cannot hold up a cancel, and notes read
/// failures so they are not taken for write failures. Interrupted reads are retried, not noted.
struct Watched<'a, R> {
    inner: R,
    cancel: &'a CancelToken,
    read_failed: &'a Cell<bool>,
}

impl<R: Read> Read for Watched<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.cancel.is_cancelled() {
            return Err(io::Error::other("cancelled"));
        }
        self.inner.read(buf).inspect_err(|err| {
            if err.kind() != io::ErrorKind::Interrupted {
                self.read_failed.set(true);
            }
        })
    }
}

enum Failure {
    Damaged(String),
    Io {
        err: io::Error,
        entry: Option<String>,
    },
}

impl Failure {
    fn io(entry: Option<&str>) -> impl FnOnce(io::Error) -> Self + '_ {
        move |err| Self::Io {
            err,
            entry: entry.map(str::to_owned),
        }
    }

    fn into_error(self, archive: &Path, into: &Path, read_failed: bool) -> AppError {
        match self {
            Self::Damaged(detail) => damaged(detail),
            Self::Io { err, entry } => match os_error(&err) {
                // The system refused, so the archive was fine as far as it was read. Only the
                // system's words are given: the archive reader's additions repeat the paths.
                Some(os) if read_failed => storage(archive, "read", described(os)),
                Some(os) => match entry {
                    Some(entry) => AppError::storage(format!(
                        "failed to unpack '{entry}' into '{}': {}",
                        into.display(),
                        described(os)
                    )),
                    None => storage(into, "unpack into", described(os)),
                },
                None => damaged(with_causes(&err)),
            },
        }
    }
}

fn damaged(detail: impl Display) -> AppError {
    AppError::model(format!("the model archive is damaged: {detail}"))
}

/// The system's own error among the causes of `err`; errors the archive reader makes up carry none.
fn os_error(err: &io::Error) -> Option<&io::Error> {
    let mut current: &(dyn Error + 'static) = err;
    loop {
        if let Some(io) = current.downcast_ref::<io::Error>()
            && io.raw_os_error().is_some()
        {
            return Some(io);
        }
        current = current.source()?;
    }
}

fn with_causes(err: &io::Error) -> String {
    let mut text = err.to_string();
    let mut cause = err.source();
    while let Some(next) = cause {
        let more = next.to_string();
        if !text.ends_with(&more) {
            text.push_str(": ");
            text.push_str(&more);
        }
        cause = next.source();
    }
    text
}

/// The system's words for `os`, with the kind of trouble added unless the words already say it.
fn described(os: &io::Error) -> String {
    let words = os.to_string();
    let kind = os.kind().to_string();
    if words.to_lowercase().contains(&kind.to_lowercase()) {
        words
    } else {
        format!("{words} ({kind})")
    }
}

/// Whether `path` is relative and has no part that leaves its folder.
fn stays_inside(path: &Path) -> bool {
    path.components()
        .all(|part| matches!(part, Component::Normal(_) | Component::CurDir))
}

/// `read_failed` is cleared before each entry, so it only describes the entry that failed.
fn unpack_entries<R: Read>(
    mut archive: Archive<R>,
    into: &Path,
    read_failed: &Cell<bool>,
) -> Result<(), Failure> {
    let refused = |detail: String| Err(Failure::Damaged(detail));
    let mut entries = archive.entries().map_err(Failure::io(None))?;
    loop {
        read_failed.set(false);
        let Some(entry) = entries.next() else {
            return Ok(());
        };
        let mut entry = entry.map_err(Failure::io(None))?;
        let kind = entry.header().entry_type();
        // The archive reader already applies headers that describe only the next entry.
        if kind == EntryType::XGlobalHeader {
            continue;
        }
        let path = entry.path().map_err(Failure::io(None))?.into_owned();
        let shown = path.display().to_string();
        let at = Some(shown.as_str());
        if !matches!(kind, EntryType::Regular | EntryType::Directory) {
            return refused(format!("'{shown}' is not an ordinary file or folder"));
        }
        if !stays_inside(&path) {
            return refused(format!("'{shown}' points outside the model's folder"));
        }
        let dest = into.join(&path);
        if kind == EntryType::Regular && dest.exists() {
            return refused(format!("'{shown}' would replace a file already there"));
        }
        if kind == EntryType::Directory {
            fs::create_dir_all(&dest).map_err(Failure::io(at))?;
            continue;
        }
        // `unpack_in` refuses entries whose path would leave `into` as well.
        if !entry.unpack_in(into).map_err(Failure::io(at))? {
            return refused(format!("'{shown}' points outside the model's folder"));
        }
        make_writable(&dest).map_err(Failure::io(at))?;
    }
}
