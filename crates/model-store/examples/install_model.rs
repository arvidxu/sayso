//! Installs a catalog model for real: `-- <model id> <folder> [stop after this many MB]`.
//! With the MB limit the download is cancelled; running again without it has to resume.

use std::cell::Cell;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use dictation_api::ModelStore;
use dictation_types::{AppError, AppResult, CancelToken, InstallPhase, ModelId};
use model_store::{LocalModelStore, ModelCatalog, ModelsConfig, resolve_local_caches};

const BYTES_PER_MB: u64 = 1_000_000;

fn main() -> ExitCode {
    let mut args: Vec<String> = env::args().skip(1).collect();
    // `--catalog <file>` installs from that catalog instead of the one the app ships with.
    let mut catalog = None;
    if args.first().is_some_and(|arg| arg == "--catalog") && args.len() > 1 {
        catalog = Some(PathBuf::from(args.remove(1)));
        args.remove(0);
    }
    let (Some(id), Some(folder)) = (args.first(), args.get(1)) else {
        eprintln!(
            "usage: install_model [--catalog <file>] <model id> <folder> [stop after this many MB]"
        );
        return ExitCode::FAILURE;
    };
    let stop_after = args.get(2).and_then(|mb| mb.parse::<u64>().ok());
    let shipped = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../model-catalog.json");
    let catalog = catalog.unwrap_or(shipped);
    match run(
        &catalog,
        &ModelId::new(id),
        PathBuf::from(folder),
        stop_after,
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("\nfailed: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run(catalog: &Path, id: &ModelId, folder: PathBuf, stop_after_mb: Option<u64>) -> AppResult<()> {
    let config = ModelsConfig::bundled()?;
    let catalog = ModelCatalog::load(catalog)?;
    let caches = resolve_local_caches(&config.local_caches);
    println!("local caches found: {}", caches.len());
    let store = LocalModelStore::new(catalog, folder, &config.download, caches)?;

    let cancel = CancelToken::new();
    let started = Instant::now();
    let first_bytes: Cell<Option<u64>> = Cell::new(None);
    let last_shown = Cell::new(u64::MAX);
    let status = store.install(
        id,
        &|progress| {
            if first_bytes.get().is_none() && progress.phase == InstallPhase::Downloading {
                first_bytes.set(Some(progress.downloaded_bytes));
                println!(
                    "starting at {} MB of {} MB",
                    progress.downloaded_bytes / BYTES_PER_MB,
                    progress.total_bytes / BYTES_PER_MB
                );
            }
            let shown = progress.downloaded_bytes / (BYTES_PER_MB * 50);
            if shown != last_shown.get() || progress.phase != InstallPhase::Downloading {
                last_shown.set(shown);
                println!(
                    "{:>6.1} s  {:?}  {} MB",
                    started.elapsed().as_secs_f32(),
                    progress.phase,
                    progress.downloaded_bytes / BYTES_PER_MB
                );
            }
            if stop_after_mb.is_some_and(|mb| progress.downloaded_bytes >= mb * BYTES_PER_MB) {
                cancel.cancel();
            }
        },
        &cancel,
    )?;
    println!("state: {:?}", status.state);
    if let Some(path) = store.local_path(id)? {
        println!("installed in {}", path.display());
        list(&path, &path)?;
    }
    Ok(())
}

fn list(base: &Path, dir: &Path) -> AppResult<()> {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|err| AppError::storage(format!("{}: {err}", dir.display())))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            list(base, &path)?;
        } else {
            let size = fs::metadata(&path).map_or(0, |meta| meta.len());
            let shown = path.strip_prefix(base).unwrap_or(&path).display();
            println!("  {size:>11}  {shown}");
        }
    }
    Ok(())
}
