use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::thread;
use std::time::Duration;

use dictation_types::{AppError, AppResult, CancelToken, ErrorKind, InstallPhase, ModelFile};
use reqwest::StatusCode;
use reqwest::blocking::{Client, Response};
use reqwest::header::{CONTENT_LENGTH, CONTENT_RANGE, HeaderMap, RANGE};

use crate::DownloadTuning;
use crate::checksum::{same_checksum, sha256_of_file};
use crate::disk::{len_of, remove_file, storage};

pub(crate) type Report<'a> = dyn FnMut(InstallPhase, u64) + 'a;

#[derive(Debug, Clone)]
pub(crate) struct Downloader {
    client: Client,
    tuning: DownloadTuning,
}

#[derive(Clone, Copy)]
struct Target<'a> {
    url: &'a str,
    file: &'a ModelFile,
    part: &'a Path,
}

enum Attempt {
    /// It still has to be verified.
    Complete,
    /// What is kept does not fit what the address has.
    StartOver(AppError),
    /// Trying the same address again may work; what arrived before the break is kept.
    Broken(AppError),
    Unusable(AppError),
}

fn cancelled() -> AppError {
    AppError::cancelled("the download was cancelled")
}

/// The first byte and the total size.
fn content_range(headers: &HeaderMap) -> Option<(u64, Option<u64>)> {
    let value = headers.get(CONTENT_RANGE)?.to_str().ok()?;
    let range = value.strip_prefix("bytes ")?;
    let (span, total) = range.split_once('/')?;
    let first = span.split('-').next()?.trim().parse().ok()?;
    Some((first, total.trim().parse().ok()))
}

fn content_length(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(CONTENT_LENGTH)?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()
}

impl Downloader {
    pub(crate) fn new(tuning: &DownloadTuning) -> AppResult<Self> {
        tuning.validate()?;
        let client = Client::builder()
            .connect_timeout(Duration::from_millis(tuning.connect_timeout_ms))
            .timeout(Duration::from_millis(tuning.read_timeout_ms))
            .build()
            .map_err(|err| AppError::network(format!("failed to set up downloading: {err}")))?;
        Ok(Self {
            client,
            tuning: tuning.clone(),
        })
    }

    pub(crate) fn buffer_bytes(&self) -> usize {
        self.tuning.buffer_bytes
    }

    pub(crate) fn unpacked_bytes(&self, archive_bytes: u64) -> u64 {
        self.tuning.unpacked_bytes(archive_bytes)
    }

    /// Size and checksum both match.
    pub(crate) fn matches(
        &self,
        path: &Path,
        file: &ModelFile,
        cancel: &CancelToken,
    ) -> AppResult<bool> {
        if !path.is_file() || len_of(path) != file.size_bytes {
            return Ok(false);
        }
        let actual = sha256_of_file(path, self.tuning.buffer_bytes, cancel)?;
        Ok(same_checksum(&actual, &file.sha256))
    }

    /// Resumes from whatever `part` holds; on success it holds the verified file, saved to disk.
    pub(crate) fn fetch(
        &self,
        file: &ModelFile,
        part: &Path,
        report: &mut Report<'_>,
        cancel: &CancelToken,
    ) -> AppResult<()> {
        let mut last_error = None;
        for url in &file.download_urls {
            match self.fetch_from(Target { url, file, part }, report, cancel) {
                Ok(()) => return Ok(()),
                // Another address cannot help when this device cannot store the file.
                Err(err) if err.is_cancelled() || err.kind == ErrorKind::Storage => {
                    return Err(err);
                }
                Err(err) => last_error = Some(err),
            }
        }
        Err(last_error.unwrap_or_else(|| {
            AppError::validation(format!("'{}' has no address to download from", file.path))
        }))
    }

    /// A try that brings more of the file than was ever on disk before resets the failed-try count.
    fn fetch_from(
        &self,
        target: Target<'_>,
        report: &mut Report<'_>,
        cancel: &CancelToken,
    ) -> AppResult<()> {
        let mut failures = 0u32;
        // A partial file longer than the file cannot be part of it.
        if len_of(target.part) > target.file.size_bytes {
            remove_file(target.part)?;
        }
        // Only getting past the most ever on disk is progress, so refetching discarded bytes
        // does not keep the tries going. Measured after an unusable partial file was removed.
        let mut furthest = len_of(target.part);
        let mut started_over_after_mismatch = false;
        let mut started_over_after_refusal = false;
        loop {
            if cancel.is_cancelled() {
                return Err(cancelled());
            }
            let error = match self.try_once(target, report, cancel)? {
                Attempt::Complete => {
                    report(InstallPhase::Verifying, target.file.size_bytes);
                    if self.matches(target.part, target.file, cancel)? {
                        return Ok(());
                    }
                    remove_file(target.part)?;
                    let mismatch = AppError::checksum_mismatch(format!(
                        "'{}' downloaded from {} does not match its checksum",
                        target.file.path, target.url
                    ));
                    // What was kept from an earlier try may have been the bad part: try once
                    // more from nothing before giving up on this address.
                    if started_over_after_mismatch {
                        return Err(mismatch);
                    }
                    started_over_after_mismatch = true;
                    // Happens once, so fetching the file again from nothing is progress.
                    furthest = 0;
                    continue;
                }
                Attempt::StartOver(err) => {
                    remove_file(target.part)?;
                    // Starting over is not the address failing: do it once uncounted.
                    if !started_over_after_refusal {
                        started_over_after_refusal = true;
                        continue;
                    }
                    err
                }
                Attempt::Broken(err) => err,
                Attempt::Unusable(err) => return Err(err),
            };
            if cancel.is_cancelled() {
                return Err(cancelled());
            }
            let have = len_of(target.part);
            if have > furthest {
                furthest = have;
                failures = 0;
            } else {
                failures += 1;
            }
            if failures >= self.tuning.attempts_per_address {
                return Err(error);
            }
            thread::sleep(Duration::from_millis(self.tuning.retry_delay_ms));
        }
    }

    fn try_once(
        &self,
        target: Target<'_>,
        report: &mut Report<'_>,
        cancel: &CancelToken,
    ) -> AppResult<Attempt> {
        let Target { url, file, part } = target;
        let have = len_of(part);
        if have == file.size_bytes {
            return Ok(Attempt::Complete);
        }
        let mut request = self.client.get(url);
        if have > 0 {
            request = request.header(RANGE, format!("bytes={have}-"));
        }
        let response = match request.send() {
            Ok(response) => response,
            Err(err) => {
                return Ok(Attempt::Broken(
                    AppError::network(format!("failed to reach {url}: {err}")).with_retryable(true),
                ));
            }
        };
        let different_file = || {
            Attempt::Unusable(AppError::network(format!(
                "{url} has a different file than the catalog describes"
            )))
        };
        let wrong_size = |total: Option<u64>| total.is_some_and(|total| total != file.size_bytes);
        let start = match response.status() {
            StatusCode::PARTIAL_CONTENT => match content_range(response.headers()) {
                Some((_, total)) if wrong_size(total) => return Ok(different_file()),
                Some((first, _)) if first == have => have,
                _ => {
                    return Ok(Attempt::StartOver(AppError::network(format!(
                        "{url} did not resume where the download stopped"
                    ))));
                }
            },
            StatusCode::OK if wrong_size(content_length(response.headers())) => {
                return Ok(different_file());
            }
            StatusCode::OK => 0,
            StatusCode::RANGE_NOT_SATISFIABLE => {
                return Ok(Attempt::StartOver(AppError::network(format!(
                    "{url} could not resume '{}'",
                    file.path
                ))));
            }
            status
                if status.is_server_error()
                    || status == StatusCode::REQUEST_TIMEOUT
                    || status == StatusCode::TOO_MANY_REQUESTS =>
            {
                return Ok(Attempt::Broken(AppError::network(format!(
                    "{url} answered {status}"
                ))));
            }
            status => {
                return Ok(Attempt::Unusable(AppError::network(format!(
                    "{url} answered {status}"
                ))));
            }
        };
        self.write_body(response, target, start, report, cancel)
    }

    fn write_body(
        &self,
        mut response: Response,
        target: Target<'_>,
        start: u64,
        report: &mut Report<'_>,
        cancel: &CancelToken,
    ) -> AppResult<Attempt> {
        let Target { url, file, part } = target;
        let mut out = if start == 0 {
            File::create(part).map_err(|err| storage(part, "create", err))?
        } else {
            OpenOptions::new()
                .append(true)
                .open(part)
                .map_err(|err| storage(part, "open", err))?
        };
        let mut written = start;
        report(InstallPhase::Downloading, written);
        let mut buffer = vec![0u8; self.tuning.buffer_bytes];
        let outcome = loop {
            if cancel.is_cancelled() {
                break Err(cancelled());
            }
            let read = match response.read(&mut buffer) {
                Ok(read) => read,
                Err(_) if cancel.is_cancelled() => break Err(cancelled()),
                Err(err) => {
                    break Ok(Attempt::Broken(
                        AppError::network(format!("the download from {url} broke off: {err}"))
                            .with_retryable(true),
                    ));
                }
            };
            if read == 0 {
                break Ok(if written < file.size_bytes {
                    Attempt::Broken(
                        AppError::network(format!(
                            "the download from {url} ended early at {written} of {} bytes",
                            file.size_bytes
                        ))
                        .with_retryable(true),
                    )
                } else {
                    Attempt::Complete
                });
            }
            let take = usize::try_from(file.size_bytes.saturating_sub(written))
                .map_or(read, |left| read.min(left));
            out.write_all(&buffer[..take])
                .map_err(|err| storage(part, "write", err))?;
            written += take as u64;
            report(InstallPhase::Downloading, written);
            if written >= file.size_bytes {
                // Anything more the server sends is not part of the file.
                break Ok(Attempt::Complete);
            }
        };
        out.flush().map_err(|err| storage(part, "write", err))?;
        out.sync_all().map_err(|err| storage(part, "save", err))?;
        outcome
    }
}
