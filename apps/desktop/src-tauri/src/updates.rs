// Downloads only after the user agrees; installs only after they agree to restart.
use std::path::PathBuf;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_store::StoreExt;
use tauri_plugin_updater::{Update, UpdaterExt};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Timing {
    first_check_seconds: u64,
    check_interval_hours: u64,
    later_hours: u64,
    // A check has to answer within this time. A download has no limit, but can be cancelled.
    check_timeout_seconds: u64,
}

pub(crate) fn timing() -> Timing {
    crate::product("updates")
}

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
struct State {
    skipped_version: Option<String>,
    downloaded_version: Option<String>,
    // "Later": timed checks stay quiet until this time, in seconds since 1970.
    offer_again_at: Option<u64>,
}

// By the clock, which keeps running while the computer sleeps and across restarts.
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

fn state_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|dir| dir.join("updates.json"))
}

// The file is outside the app bundle, so content that does not parse counts as no state.
fn read_state(app: &AppHandle) -> State {
    state_path(app)
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

// One change at a time, so that two threads cannot undo each other's.
fn save_state(app: &AppHandle, change: impl FnOnce(&mut State)) {
    static SAVING: Mutex<()> = Mutex::new(());
    let _saving = SAVING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut state = read_state(app);
    change(&mut state);
    let Some(path) = state_path(app) else {
        return;
    };
    let saved = serde_json::to_vec(&state)
        .map_err(std::io::Error::other)
        .and_then(|json| {
            path.parent().map_or(Ok(()), std::fs::create_dir_all)?;
            // Written beside it and then moved over it, so it is never read half written.
            let partial = path.with_extension("partial");
            std::fs::write(&partial, json)?;
            std::fs::rename(&partial, &path)
        });
    if let Err(error) = saved {
        eprintln!("Could not save the update state: {error}");
    }
}

fn package_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_cache_dir()
        .ok()
        .map(|dir| dir.join("update"))
}

#[derive(Default)]
struct Session {
    restart_offered_for: Option<String>,
    update: Option<Update>,
    // Only when the package could not be saved to disk.
    package: Option<Vec<u8>>,
    is_busy: bool,
    cancel_download: Option<Box<dyn Fn() + Send>>,
    // What a timed check found before the page was listening.
    pending: Option<Outcome>,
}

struct Busy<'a>(&'a AppHandle);

impl<'a> Busy<'a> {
    fn begin(app: &'a AppHandle) -> Option<Self> {
        let mut session = session(app);
        if session.is_busy {
            return None;
        }
        session.is_busy = true;
        Some(Self(app))
    }
}

impl Drop for Busy<'_> {
    fn drop(&mut self) {
        let mut session = session(self.0);
        session.is_busy = false;
        session.cancel_download = None;
    }
}

#[derive(Clone, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum Outcome {
    Unavailable,
    Busy,
    Latest {
        version: String,
    },
    Available {
        version: String,
        notes: Option<String>,
    },
    Ready {
        version: String,
        notes: Option<String>,
    },
    Failed {
        message: String,
    },
}

// Only release builds get the updater configuration; development builds never check.
fn configured(app: &AppHandle) -> bool {
    app.config().plugins.0.contains_key("updater")
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    app.manage(Mutex::new(Session::default()));
    if !configured(app) {
        return Ok(());
    }
    app.plugin(tauri_plugin_updater::Builder::new().build())?;
    let timing = timing();
    let app = app.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(timing.first_check_seconds));
        loop {
            if is_automatic(&app) && !session(&app).is_busy {
                match tauri::async_runtime::block_on(find_timed(&app)) {
                    // Also kept for a page that is not listening yet.
                    Ok(Some(outcome)) => {
                        session(&app).pending = Some(outcome.clone());
                        if let Err(error) = app.emit("update-found", outcome) {
                            eprintln!("Could not report the update: {error}");
                        }
                    }
                    Ok(None) => {}
                    Err(error) => eprintln!("Update check failed: {error}"),
                }
            }
            // In short naps, so that time the computer spent asleep counts.
            let next = now() + timing.check_interval_hours * 60 * 60;
            while now() < next {
                thread::sleep(Duration::from_secs(60));
            }
        }
    });
    Ok(())
}

// Only gates timed checks; a check the user asks for always runs.
fn is_automatic(app: &AppHandle) -> bool {
    app.store(crate::SETTINGS)
        .ok()
        .and_then(|settings| settings.get("automaticUpdates"))
        .and_then(|value| value.as_bool())
        .unwrap_or_else(|| crate::app_config().initial_settings.automatic_updates)
}

fn session(app: &AppHandle) -> std::sync::MutexGuard<'_, Session> {
    app.state::<Mutex<Session>>()
        .inner()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

async fn fetch(app: &AppHandle) -> tauri_plugin_updater::Result<Option<Update>> {
    let timeout = Duration::from_secs(timing().check_timeout_seconds);
    let updater = app.updater_builder().timeout(timeout).build()?;
    let mut update = updater.check().await?;
    // The time limit is for the check; the download takes as long as it takes.
    if let Some(update) = &mut update {
        update.timeout = None;
    }
    Ok(update)
}

// Removes a package of another version or one that fails the signature check. Call only while no
// download runs, so it cannot remove one being written.
fn is_downloaded(app: &AppHandle, update: &Update, state: &State) -> bool {
    let Some(downloaded) = &state.downloaded_version else {
        return false;
    };
    let is_usable = *downloaded == update.version
        && package_path(app)
            .and_then(|path| std::fs::read(path).ok())
            .is_some_and(|bytes| update.verify(&bytes).is_ok());
    if !is_usable {
        forget_download(app);
    }
    is_usable
}

fn offer(session: &mut Session, update: Update, is_downloaded: bool) -> Outcome {
    let version = update.version.clone();
    let notes = update.body.clone();
    session.update = Some(update);
    session.package = None;
    if is_downloaded {
        // The restart question comes once per version in each launch.
        session.restart_offered_for = Some(version.clone());
        Outcome::Ready { version, notes }
    } else {
        Outcome::Available { version, notes }
    }
}

async fn find_timed(app: &AppHandle) -> tauri_plugin_updater::Result<Option<Outcome>> {
    let Some(update) = fetch(app).await? else {
        return Ok(None);
    };
    let mut session = session(app);
    // It leaves alone what the user began meanwhile; holding the session, nothing begins now.
    if session.is_busy || session.restart_offered_for.as_deref() == Some(&update.version) {
        return Ok(None);
    }
    let state = read_state(app);
    if state.skipped_version.as_deref() == Some(&update.version) {
        return Ok(None);
    }
    let is_downloaded = is_downloaded(app, &update, &state);
    // "Later" holds back a version that is not downloaded yet.
    if !is_downloaded && state.offer_again_at.is_some_and(|at| now() < at) {
        return Ok(None);
    }
    Ok(Some(offer(&mut session, update, is_downloaded)))
}

// Unlike a timed check, offers a skipped or held-back version again.
async fn find_asked(app: &AppHandle) -> tauri_plugin_updater::Result<Outcome> {
    let Some(update) = fetch(app).await? else {
        return Ok(Outcome::Latest {
            version: app.package_info().version.to_string(),
        });
    };
    let is_downloaded = is_downloaded(app, &update, &read_state(app));
    Ok(offer(&mut session(app), update, is_downloaded))
}

fn failed(error: impl std::fmt::Display) -> Outcome {
    Outcome::Failed {
        message: error.to_string(),
    }
}

pub async fn check_now(app: &AppHandle) -> Outcome {
    if !configured(app) {
        return Outcome::Unavailable;
    }
    let Some(_busy) = Busy::begin(app) else {
        return Outcome::Busy;
    };
    session(app).pending = None;
    match find_asked(app).await {
        Ok(outcome) => outcome,
        // The newest release has no build for this system, so there is nothing newer to install.
        Err(
            tauri_plugin_updater::Error::TargetNotFound(_)
            | tauri_plugin_updater::Error::TargetsNotFound(_),
        ) => Outcome::Latest {
            version: app.package_info().version.to_string(),
        },
        Err(error) => failed(error),
    }
}

pub async fn download(app: &AppHandle) -> Outcome {
    let Some(_busy) = Busy::begin(app) else {
        return Outcome::Busy;
    };
    let Some(update) = session(app).update.clone() else {
        return failed("no update to download");
    };
    // In a task of its own, so that `cancel_download` can stop it.
    let task = {
        let update = update.clone();
        tauri::async_runtime::spawn(async move { update.download(|_, _| {}, || {}).await })
    };
    let abort = task.inner().abort_handle();
    session(app).cancel_download = Some(Box::new(move || abort.abort()));
    let bytes = match task.await {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(error)) => return failed(error),
        // Cancelled; the page has already closed the dialog.
        Err(error) => return failed(error),
    };
    // Kept on disk until the restart, which may be in a later launch, and not in memory as well.
    let saved = package_path(app)
        .ok_or_else(|| std::io::Error::other("no cache folder"))
        .and_then(|path| {
            path.parent().map_or(Ok(()), std::fs::create_dir_all)?;
            std::fs::write(&path, &bytes)
        });
    let kept_in_memory = match saved {
        Ok(()) => {
            save_state(app, |state| {
                state.downloaded_version = Some(update.version.clone());
            });
            None
        }
        Err(error) => {
            eprintln!("Could not keep the downloaded update: {error}");
            Some(bytes)
        }
    };
    let mut session = session(app);
    session.restart_offered_for = Some(update.version.clone());
    session.package = kept_in_memory;
    Outcome::Ready {
        version: update.version.clone(),
        notes: update.body.clone(),
    }
}

pub fn cancel_download(app: &AppHandle) {
    if let Some(cancel) = session(app).cancel_download.take() {
        cancel();
    }
}

pub fn take_pending(app: &AppHandle) -> Option<Outcome> {
    session(app).pending.take()
}

fn forget_download(app: &AppHandle) {
    if let Some(path) = package_path(app) {
        let _ = std::fs::remove_file(path);
    }
    save_state(app, |state| state.downloaded_version = None);
}

// Returns only when the installation failed. Blocks, so call it off the main thread.
pub fn install(app: &AppHandle) -> Outcome {
    let Some(_busy) = Busy::begin(app) else {
        return Outcome::Busy;
    };
    let (update, in_memory) = {
        let mut session = session(app);
        (session.update.clone(), session.package.take())
    };
    let Some(update) = update else {
        return failed("no downloaded update to install");
    };
    // Checked again against the signature, as the file is outside the app bundle.
    let on_disk = || {
        package_path(app)
            .and_then(|path| std::fs::read(path).ok())
            .filter(|bytes| update.verify(bytes).is_ok())
    };
    let Some(bytes) = in_memory.or_else(on_disk) else {
        return failed("no downloaded update to install");
    };
    // Here the installer closes the app and restarts it, so nothing runs after it.
    #[cfg(windows)]
    forget_download(app);
    if let Err(error) = update.install(&bytes) {
        session(app).package = Some(bytes);
        return failed(error);
    }
    #[cfg(not(windows))]
    forget_download(app);
    app.restart();
}

pub fn skip(app: &AppHandle) {
    let version = session(app)
        .update
        .as_ref()
        .map(|update| update.version.clone());
    if let Some(version) = version {
        save_state(app, |state| state.skipped_version = Some(version));
    }
    if let Some(_busy) = Busy::begin(app) {
        forget_download(app);
    }
}

pub fn postpone(app: &AppHandle) {
    let at = now() + timing().later_hours * 60 * 60;
    save_state(app, |state| state.offer_again_at = Some(at));
}

pub fn forget_skipped(app: &AppHandle) {
    save_state(app, |state| {
        state.skipped_version = None;
        state.offer_again_at = None;
    });
}
