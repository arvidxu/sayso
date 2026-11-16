use std::io::{self, BufRead, Write};
use std::path::Path;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use asr_local::{ModelBackend, SpeechModel};
use dictation_types::{AppError, AppResult, CancelToken};

use crate::wire::{
    LoadRequest, REPLY_MARK, RecogniseRequest, Reply, Request, WireError, parse_bytes,
    read_raw_line, read_samples, write_line,
};

/// Returns when the main program closes its end. Once a model is loading, the process also
/// exits when the main program is gone, even while an engine is stuck.
pub fn serve(
    backends: &[Arc<dyn ModelBackend>],
    mut input: impl BufRead,
    mut output: impl Write,
) -> AppResult<()> {
    let started_by = parent();
    let mut loaded: Option<Box<dyn SpeechModel>> = None;
    while let Some(line) = read_raw_line(&mut input)? {
        let reply = match parse_bytes::<Request>(&line)? {
            // The main program sends one load per worker.
            Request::Load(request) => {
                end_with_parent(started_by, request.worker_parent_check_ms);
                match load(backends, *request) {
                    Ok(model) => {
                        loaded = Some(model);
                        Reply::Loaded
                    }
                    Err(err) => Reply::Failed(WireError::from(&err)),
                }
            }
            Request::Recognise(request) => {
                // The samples are read even when the request cannot be served, so the next
                // request starts where it should.
                let samples = read_samples(&mut input, request.samples)?;
                match recognise(loaded.as_deref(), &request, &samples) {
                    Ok(text) => Reply::Text(text),
                    Err(err) => Reply::Failed(WireError::from(&err)),
                }
            }
        };
        write_line(&mut output, REPLY_MARK, &reply)?;
    }
    Ok(())
}

/// The main program checked the tuning and the entry with the same engine before sending them.
fn load(
    backends: &[Arc<dyn ModelBackend>],
    request: LoadRequest,
) -> AppResult<Box<dyn SpeechModel>> {
    let tuning = request.tuning();
    let model = request.model.into_descriptor()?;
    let backend = backends
        .iter()
        .find(|backend| backend.engine_id() == model.engine)
        .ok_or_else(|| {
            AppError::unsupported(format!(
                "this worker was built without the engine '{}'",
                model.engine
            ))
        })?;
    // Cancelling is done by ending the worker, so nothing here is ever cancelled.
    backend.load(
        &model,
        Path::new(&request.dir),
        &tuning,
        &CancelToken::new(),
    )
}

fn recognise(
    model: Option<&dyn SpeechModel>,
    request: &RecogniseRequest,
    samples: &[f32],
) -> AppResult<String> {
    let model = model.ok_or_else(|| AppError::asr("no model is loaded in this worker"))?;
    model.recognise(
        samples,
        request.sample_rate_hz,
        &request.options()?,
        &CancelToken::new(),
    )
}

/// Orphaned processes get a new parent, so a changed parent id means ours has gone. Where
/// this is unavailable the client puts the worker in a job ended with the main program.
fn parent() -> Option<u32> {
    #[cfg(unix)]
    return Some(std::os::unix::process::parent_id());
    #[cfg(not(unix))]
    return None;
}

fn end_with_parent(started_by: Option<u32>, every_ms: u64) {
    let Some(started_by) = started_by else {
        return;
    };
    thread::spawn(move || {
        while parent() == Some(started_by) {
            thread::sleep(Duration::from_millis(every_ms));
        }
        std::process::exit(1);
    });
}

/// Moves standard output to the error output so text an engine prints cannot corrupt the
/// replies. Call it before any engine is loaded.
#[cfg(unix)]
pub fn reply_output() -> io::Result<Box<dyn Write>> {
    use std::os::fd::FromRawFd;
    io::stdout().flush()?;
    // SAFETY: calls on the process's own descriptors; the `dup` copy is owned by the file.
    unsafe {
        let replies = libc::dup(libc::STDOUT_FILENO);
        if replies < 0 {
            return Err(io::Error::last_os_error());
        }
        let replies = std::fs::File::from_raw_fd(replies);
        if libc::dup2(libc::STDERR_FILENO, libc::STDOUT_FILENO) < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Box::new(io::BufWriter::new(replies)))
    }
}

/// Not separated here: engine text shares the stream and is told apart by the reply mark.
#[cfg(not(unix))]
pub fn reply_output() -> io::Result<Box<dyn Write>> {
    Ok(Box::new(io::stdout()))
}
