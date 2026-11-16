use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use dictation_types::{
    AppError, AppResult, CancelToken, DownloadProgress, InstallPhase, ModelDescriptor, ModelFile,
    ModelId, ModelStatus,
};

use crate::archive;
use crate::current_platform;
use crate::disk::{
    copy_file, create_dir, create_parent, ensure_room, len_of, link_or_copy, remove_dir,
    remove_file, storage,
};
use crate::local_cache::find_by_checksum;
use crate::store::{Claim, LocalModelStore};
use crate::transfer::Report;

fn part_path(dest: &Path) -> PathBuf {
    let mut name: OsString = dest.as_os_str().to_owned();
    name.push(".part");
    PathBuf::from(name)
}

impl LocalModelStore {
    fn refuse_unusable(descriptor: &ModelDescriptor) -> AppResult<()> {
        if descriptor.provided_by_system {
            return Err(AppError::unsupported(format!(
                "model {} is provided by the system and has nothing to install",
                descriptor.id
            )));
        }
        let here = current_platform();
        if !here.is_some_and(|platform| descriptor.supported_platforms.contains(&platform)) {
            return Err(AppError::unsupported(format!(
                "model {} does not run on this system",
                descriptor.id
            )));
        }
        Ok(())
    }

    pub(crate) fn install_model(
        &self,
        id: &ModelId,
        progress: &dyn Fn(&DownloadProgress),
        cancel: &CancelToken,
    ) -> AppResult<ModelStatus> {
        let entry = self.entry(id)?;
        Self::refuse_unusable(entry)?;
        let claim = Claim::take(self, id)?;
        // An installed model is not downloaded, so it never reports as downloading.
        if !self.layout.is_installed(entry) {
            claim.report_downloading(entry.size_bytes());
            let _unused = self.lock_unused(id)?;
            self.ensure_room_for(entry)?;
            self.fetch_all(entry, progress, cancel)?;
            self.assemble(entry, progress, cancel)?;
        }
        self.state().corrupted.remove(id);
        drop(claim);
        Ok(self.status_of(entry))
    }

    /// Archive content size is not in the catalog, so it is estimated with `unpacked_size_ratio`;
    /// passing this check is no promise.
    fn ensure_room_for(&self, entry: &ModelDescriptor) -> AppResult<()> {
        let downloads = self.layout.downloads_dir(&entry.id);
        let mut needed = 0u64;
        for file in &entry.files {
            let dest = downloads.join(&file.path);
            let have = len_of(&dest).max(len_of(&part_path(&dest)));
            needed = needed.saturating_add(file.size_bytes.saturating_sub(have));
            if file.unpack.is_some() {
                needed = needed.saturating_add(self.downloader.unpacked_bytes(file.size_bytes));
            }
        }
        let partial = self.layout.partial_dir(&entry.id);
        create_dir(&partial)?;
        let room = ensure_room(&partial, needed);
        if room.is_err() {
            // Only removes the folder when it is empty, as it is when just made.
            let _ = fs::remove_dir(&partial);
        }
        room
    }

    /// Reuses verified files an earlier attempt left in the downloads folder.
    fn fetch_all(
        &self,
        entry: &ModelDescriptor,
        progress: &dyn Fn(&DownloadProgress),
        cancel: &CancelToken,
    ) -> AppResult<()> {
        let downloads = self.layout.downloads_dir(&entry.id);
        let total = entry.size_bytes();
        let mut finished = 0u64;
        for file in &entry.files {
            let mut report = |phase: InstallPhase, bytes: u64| {
                let downloaded = finished.saturating_add(bytes).min(total);
                if let Some(entry_progress) = self.state().downloading.get_mut(&entry.id) {
                    *entry_progress = (downloaded, total);
                }
                progress(&DownloadProgress {
                    model_id: entry.id.clone(),
                    phase,
                    file_path: file.path.clone(),
                    downloaded_bytes: downloaded,
                    total_bytes: total,
                });
            };
            let dest = downloads.join(&file.path);
            create_parent(&dest)?;
            if len_of(&dest) > 0 {
                report(InstallPhase::Verifying, 0);
            }
            if !self.downloader.matches(&dest, file, cancel)? {
                remove_file(&dest)?;
                self.fetch_one(file, &dest, &mut report, cancel)?;
            }
            report(InstallPhase::Downloading, file.size_bytes);
            finished = finished.saturating_add(file.size_bytes);
        }
        Ok(())
    }

    /// Prefers a local cache; the download addresses are used when none has the file or the
    /// cached copy cannot be read or does not match.
    fn fetch_one(
        &self,
        file: &ModelFile,
        dest: &Path,
        report: &mut Report<'_>,
        cancel: &CancelToken,
    ) -> AppResult<()> {
        let part = part_path(dest);
        let mut ready = false;
        if len_of(&part) == 0
            && let Some(cached) = find_by_checksum(&self.caches, &file.sha256)
            && len_of(&cached) == file.size_bytes
        {
            let copied = copy_file(
                &cached,
                &part,
                self.downloader.buffer_bytes(),
                &mut |bytes| report(InstallPhase::Downloading, bytes),
                cancel,
            );
            match copied {
                Err(err) if err.is_cancelled() => return Err(err),
                // `copy_file` removed what it had copied.
                Err(_) => {}
                Ok(()) => {
                    report(InstallPhase::Verifying, file.size_bytes);
                    ready = self.downloader.matches(&part, file, cancel)?;
                    if !ready {
                        remove_file(&part)?;
                    }
                }
            }
        }
        if !ready {
            self.downloader.fetch(file, &part, report, cancel)?;
        }
        fs::rename(&part, dest).map_err(|err| storage(dest, "move into", err))
    }

    /// Rebuilt from scratch out of the verified downloads every time.
    fn assemble(
        &self,
        entry: &ModelDescriptor,
        progress: &dyn Fn(&DownloadProgress),
        cancel: &CancelToken,
    ) -> AppResult<()> {
        let assembled = self.layout.assembled_dir(&entry.id);
        remove_dir(&assembled)?;
        let built = self.build(entry, &assembled, progress, cancel);
        if built.is_err() {
            // The downloads stay for the next try.
            let _ = remove_dir(&assembled);
        }
        built
    }

    fn build(
        &self,
        entry: &ModelDescriptor,
        assembled: &Path,
        progress: &dyn Fn(&DownloadProgress),
        cancel: &CancelToken,
    ) -> AppResult<()> {
        let downloads = self.layout.downloads_dir(&entry.id);
        create_dir(assembled)?;
        let total = entry.size_bytes();
        for file in entry.files.iter().filter(|file| file.unpack.is_none()) {
            let dest = assembled.join(&file.path);
            create_parent(&dest)?;
            link_or_copy(
                &downloads.join(&file.path),
                &dest,
                self.downloader.buffer_bytes(),
            )?;
        }
        for file in &entry.files {
            if let Some(format) = file.unpack {
                progress(&DownloadProgress {
                    model_id: entry.id.clone(),
                    phase: InstallPhase::Unpacking,
                    file_path: file.path.clone(),
                    downloaded_bytes: total,
                    total_bytes: total,
                });
                archive::unpack(&downloads.join(&file.path), format, assembled, cancel)?;
            }
        }
        self.layout.finish(entry)?;
        Ok(())
    }

    pub(crate) fn import_model(&self, id: &ModelId, from: &Path) -> AppResult<ModelStatus> {
        let entry = self.entry(id)?;
        Self::refuse_unusable(entry)?;
        let _claim = Claim::take(self, id)?;
        let _unused = self.lock_unused(id)?;
        for file in &entry.files {
            if !from.join(&file.path).is_file() {
                return Err(AppError::not_found(format!(
                    "'{}' is missing from '{}'",
                    file.path,
                    from.display()
                )));
            }
        }
        self.ensure_room_for(entry)?;
        let never = CancelToken::new();
        let downloads = self.layout.downloads_dir(id);
        for file in &entry.files {
            let dest = downloads.join(&file.path);
            create_parent(&dest)?;
            // `dest` may still be linked to the installed file, which must not be overwritten.
            remove_file(&dest)?;
            copy_file(
                &from.join(&file.path),
                &dest,
                self.downloader.buffer_bytes(),
                &mut |_| {},
                &never,
            )?;
            // Check the copy, not the source, which may change while it is copied.
            if !self.downloader.matches(&dest, file, &never)? {
                remove_file(&dest)?;
                return Err(self.mismatch(entry, file));
            }
        }
        self.assemble(entry, &|_| {}, &never)?;
        self.state().corrupted.remove(id);
        drop(_claim);
        Ok(self.status_of(entry))
    }

    /// A model that is not installed is marked corrupted; an installed model stays as it is.
    fn mismatch(&self, entry: &ModelDescriptor, file: &ModelFile) -> AppError {
        let reason = format!("'{}' does not match its checksum", file.path);
        if !self.layout.is_installed(entry) {
            self.state()
                .corrupted
                .insert(entry.id.clone(), reason.clone());
        }
        AppError::checksum_mismatch(format!("model {}: {reason}", entry.id))
    }
}
