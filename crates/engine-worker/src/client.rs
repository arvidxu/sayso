use std::env::{consts::EXE_SUFFIX, current_exe};
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use asr_local::{ModelBackend, RecognitionTuning, SpeechModel};
use dictation_types::{AppError, AppResult, AsrOptions, CancelToken, ModelDescriptor, ProviderId};

use crate::wire::{LoadRequest, RecogniseRequest, Reply, Request, WireModel};
use crate::worker::{Pipes, Worker, lock, stop};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerCommand {
    pub program: PathBuf,
}

/// Without the platform's executable suffix.
const WORKER_PROGRAM: &str = "engine-worker";

impl WorkerCommand {
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
        }
    }

    /// Where an installed application keeps it. Fails when it is not there.
    pub fn beside_current_program() -> AppResult<Self> {
        let me = current_exe().map_err(|err| {
            AppError::internal(format!("this program cannot tell where it is: {err}"))
        })?;
        let program = me
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(format!("{WORKER_PROGRAM}{EXE_SUFFIX}"));
        if !program.is_file() {
            return Err(AppError::not_found(format!(
                "the recognition worker is not at '{}'",
                program.display()
            )));
        }
        Ok(Self::new(program))
    }
}

/// The way an application should run engines: nothing an engine does to its process
/// reaches the application.
pub fn isolated_backends(command: &WorkerCommand) -> Vec<Arc<dyn ModelBackend>> {
    asr_local::built_in_backends()
        .into_iter()
        .map(|engine| -> Arc<dyn ModelBackend> {
            Arc::new(WorkerBackend::new(engine, command.clone()))
        })
        .collect()
}

/// One worker process per loaded model. A crash or hang shows up as a failed load or
/// recognition; the next load starts a fresh worker. Cancelling ends the worker.
pub struct WorkerBackend {
    engine: Arc<dyn ModelBackend>,
    command: WorkerCommand,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ended {
    Cancelled,
    TimedOut,
}

/// Does nothing if the worker was already ended for another reason.
fn end_for(
    why: Ended,
    ended: &Arc<Mutex<Option<Ended>>>,
    process: &Arc<Mutex<Child>>,
) -> impl FnOnce() + Send + 'static {
    let (ended, process) = (Arc::clone(ended), Arc::clone(process));
    move || {
        let mut ended = lock(&ended);
        if ended.is_none() {
            *ended = Some(why);
            // Fails only when the worker already ended, which is what was wanted.
            lock(&process).kill().ok();
        }
    }
}

impl WorkerBackend {
    /// `engine` only checks catalog entries here; models load in the worker, which must be
    /// built with the same engine.
    pub fn new(engine: Arc<dyn ModelBackend>, command: WorkerCommand) -> Self {
        Self { engine, command }
    }
}

/// The worker is ended on cancel or load timeout; the second value says which, if either.
fn load_in(
    worker: &mut Worker,
    request: &Request,
    tuning: &RecognitionTuning,
    cancel: &CancelToken,
) -> (AppResult<Option<Reply>>, Option<Ended>) {
    let ended = Arc::new(Mutex::new(None));
    let watch = cancel.on_cancel(end_for(Ended::Cancelled, &ended, &worker.process));
    let time_out = end_for(Ended::TimedOut, &ended, &worker.process);
    let limit = Duration::from_millis(tuning.load_timeout_ms);
    let answered = (Mutex::new(false), Condvar::new());
    let reply = thread::scope(|scope| {
        let answered = &answered;
        scope.spawn(move || {
            let (done, arrived) = answered;
            let (guard, waited) = arrived
                .wait_timeout_while(lock(done), limit, |done| !*done)
                .unwrap_or_else(PoisonError::into_inner);
            drop(guard);
            if waited.timed_out() {
                time_out();
            }
        });
        let reply = worker
            .pipes
            .send(request, &[])
            .and_then(|()| worker.pipes.reply());
        *lock(&answered.0) = true;
        answered.1.notify_all();
        reply
    });
    drop(watch);
    let why = *lock(&ended);
    (reply, why)
}

impl ModelBackend for WorkerBackend {
    fn engine_id(&self) -> ProviderId {
        self.engine.engine_id()
    }

    fn check(&self, model: &ModelDescriptor) -> AppResult<()> {
        self.engine.check(model)
    }

    fn load(
        &self,
        model: &ModelDescriptor,
        dir: &Path,
        tuning: &RecognitionTuning,
        cancel: &CancelToken,
    ) -> AppResult<Box<dyn SpeechModel>> {
        let dir = dir.to_str().ok_or_else(|| {
            AppError::validation("the model's folder has a name that cannot be passed on")
        })?;
        let request = Request::Load(Box::new(LoadRequest {
            model: WireModel::from(model),
            dir: dir.to_owned(),
            thread_share: tuning.thread_share,
            execution_provider: tuning.execution_provider.clone(),
            load_timeout_ms: tuning.load_timeout_ms,
            worker_parent_check_ms: tuning.worker_parent_check_ms,
        }));
        let stopped = || AppError::cancelled(format!("loading model {} was stopped", model.id));
        if cancel.is_cancelled() {
            return Err(stopped());
        }
        let mut worker = Worker::start(&self.command)?;
        let (reply, ended) = load_in(&mut worker, &request, tuning, cancel);
        match ended {
            Some(Ended::Cancelled) => {
                stop(&worker.process);
                return Err(stopped());
            }
            Some(Ended::TimedOut) => {
                stop(&worker.process);
                return Err(AppError::timeout(format!(
                    "model {} was not loaded within {} ms",
                    model.id, tuning.load_timeout_ms
                )));
            }
            None => {}
        }
        match reply {
            Ok(Some(Reply::Loaded)) => Ok(Box::new(WorkerModel {
                pipes: Mutex::new(worker.pipes),
                process: worker.process,
                usable: Arc::new(AtomicBool::new(true)),
            })),
            Ok(Some(Reply::Failed(err))) => {
                stop(&worker.process);
                Err(err.into())
            }
            Ok(Some(Reply::Text(_))) => {
                stop(&worker.process);
                Err(AppError::internal("the worker answered a load with text"))
            }
            Ok(None) => Err(AppError::asr(format!(
                "the recognition engine stopped while loading model {} ({}); its files may be \
                 damaged or not what the catalog says",
                model.id,
                stop(&worker.process)
            ))),
            Err(err) => {
                stop(&worker.process);
                Err(err)
            }
        }
    }
}

/// Dropping it ends the worker.
struct WorkerModel {
    pipes: Mutex<Pipes>,
    process: Arc<Mutex<Child>>,
    /// Cleared once the worker is gone, or the moment a cancel ends it.
    usable: Arc<AtomicBool>,
}

impl WorkerModel {
    fn worker_gone(&self, cause: &str) -> AppError {
        self.usable.store(false, Ordering::SeqCst);
        let ended = stop(&self.process);
        AppError::asr(format!(
            "the recognition engine stopped in the middle of a recognition ({cause}; {ended})"
        ))
    }
}

impl SpeechModel for WorkerModel {
    fn recognise(
        &self,
        samples: &[f32],
        sample_rate_hz: u32,
        opts: &AsrOptions,
        cancel: &CancelToken,
    ) -> AppResult<String> {
        let request =
            Request::Recognise(RecogniseRequest::new(samples.len(), sample_rate_hz, opts));
        let mut pipes = lock(&self.pipes);
        // Cancelled while queued behind another recognition: it never reached the worker.
        if cancel.is_cancelled() {
            return Err(AppError::cancelled("the recognition was stopped"));
        }
        if !self.is_usable() {
            return Err(AppError::asr("the recognition worker is not running"));
        }
        // Only the recognition holding the pipes has a watch, so a cleared flag after the watch
        // is dropped means this recognition's cancel ended the worker.
        let watch = {
            let (usable, process) = (Arc::clone(&self.usable), Arc::clone(&self.process));
            cancel.on_cancel(move || {
                usable.store(false, Ordering::SeqCst);
                // Fails only when the worker already ended, which is what was wanted.
                lock(&process).kill().ok();
            })
        };
        let reply = if self.usable.load(Ordering::SeqCst) {
            pipes.send(&request, samples).and_then(|()| pipes.reply())
        } else {
            Ok(None)
        };
        drop(watch);
        if !self.usable.load(Ordering::SeqCst) {
            // Ended, perhaps just after it answered: it cannot be used again.
            stop(&self.process);
            return Err(AppError::cancelled("the recognition was stopped"));
        }
        match reply {
            Ok(Some(Reply::Text(text))) => Ok(text),
            Ok(Some(Reply::Failed(err))) => Err(err.into()),
            Ok(Some(Reply::Loaded)) => {
                Err(self.worker_gone("it answered a recognition with a load"))
            }
            Ok(None) => Err(self.worker_gone("the worker ended")),
            // A broken pipe or an out-of-step reply: the worker cannot be trusted again.
            Err(err) => Err(self.worker_gone(&err.to_string())),
        }
    }

    /// Also asks the system, so a worker that ended while idle is noticed before use.
    fn is_usable(&self) -> bool {
        if self.usable.load(Ordering::SeqCst) && !matches!(lock(&self.process).try_wait(), Ok(None))
        {
            self.usable.store(false, Ordering::SeqCst);
        }
        self.usable.load(Ordering::SeqCst)
    }
}

impl Drop for WorkerModel {
    fn drop(&mut self) {
        stop(&self.process);
    }
}
