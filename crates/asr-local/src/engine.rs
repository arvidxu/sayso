use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

use dictation_api::{AsrEngine, AsrSession, Clock, ModelStore};
use dictation_types::{
    AppError, AppResult, AsrCaps, AsrOptions, CancelToken, ModelDescriptor, ProviderId,
    RequestRoute, StageTimings, Transcript, TranscriptId, TranscriptSegment,
};

use crate::session::BufferedSession;
use crate::{ModelBackend, RecognitionTuning, SpeechModel, capabilities_of};

const STAGE_RECOGNITION: &str = "asr";

const STAGE_MODEL_LOAD: &str = "model_load";

#[derive(Default)]
struct ModelState {
    model: Option<Arc<dyn SpeechModel>>,
    /// Set while one thread loads; the others wait on `load_finished`.
    loading: bool,
    /// Running recognitions; the model is not unloaded while this is non-zero.
    in_use: usize,
    /// A token per unfinished recognition or load, registered when requested, for `interrupt`.
    running: Vec<CancelToken>,
    last_used_ms: u64,
}

/// Dropping it lets the model be unloaded again.
struct InUse<'a> {
    engine: &'a Inner,
    model: Arc<dyn SpeechModel>,
}

impl Drop for InUse<'_> {
    fn drop(&mut self) {
        let now = self.engine.clock.monotonic_ms();
        let mut state = self.engine.state();
        state.in_use = state.in_use.saturating_sub(1);
        state.last_used_ms = now;
    }
}

/// Clears `loading` and wakes waiters even if the load panics.
struct LoadEnds<'a>(&'a Inner);

impl Drop for LoadEnds<'_> {
    fn drop(&mut self) {
        self.0.state().loading = false;
        self.0.load_finished.notify_all();
    }
}

pub(crate) struct Inner {
    backend: Arc<dyn ModelBackend>,
    store: Arc<dyn ModelStore>,
    clock: Arc<dyn Clock>,
    model: ModelDescriptor,
    pub(crate) caps: AsrCaps,
    tuning: RecognitionTuning,
    state: Mutex<ModelState>,
    load_finished: Condvar,
    transcripts: AtomicU64,
}

/// Loads its model on first use and keeps it until [`unload_idle`](Self::unload_idle) or
/// [`unload`](Self::unload).
#[derive(Clone)]
pub struct LocalAsrEngine {
    inner: Arc<Inner>,
}

impl LocalAsrEngine {
    /// The model does not have to be installed yet.
    pub fn new(
        backend: Arc<dyn ModelBackend>,
        store: Arc<dyn ModelStore>,
        clock: Arc<dyn Clock>,
        model: ModelDescriptor,
        tuning: RecognitionTuning,
    ) -> AppResult<Self> {
        tuning.validate()?;
        if model.engine != backend.engine_id() {
            return Err(AppError::validation(format!(
                "model {} is made for the engine '{}', not '{}'",
                model.id,
                model.engine,
                backend.engine_id()
            )));
        }
        let caps = capabilities_of(&model)?;
        backend.check(&model)?;
        Ok(Self {
            inner: Arc::new(Inner {
                backend,
                store,
                clock,
                model,
                caps,
                tuning,
                state: Mutex::new(ModelState::default()),
                load_finished: Condvar::new(),
                transcripts: AtomicU64::new(0),
            }),
        })
    }

    pub fn is_loaded(&self) -> bool {
        self.inner.state().model.is_some()
    }

    pub fn load(&self) -> AppResult<()> {
        let running = self.inner.start_running();
        self.inner.hold_model(&running.token).map(|_| ())
    }

    /// Refused while a recognition or load is running. Returns whether it was loaded.
    pub fn unload(&self) -> AppResult<bool> {
        self.inner.unload_if(|_| true)
    }

    /// Returns whether it was unloaded.
    pub fn unload_idle(&self, idle_timeout_ms: u64) -> AppResult<bool> {
        let now = self.inner.clock.monotonic_ms();
        self.inner.unload_if(|state| {
            state.in_use == 0
                && !state.loading
                && now.saturating_sub(state.last_used_ms) >= idle_timeout_ms
        })
    }
}

/// Dropping it removes its token from the list `interrupt` reaches.
struct Running<'a> {
    engine: &'a Inner,
    token: CancelToken,
}

impl Drop for Running<'_> {
    fn drop(&mut self) {
        let token = &self.token;
        self.engine
            .state()
            .running
            .retain(|other| !other.is_same(token));
    }
}

fn cancelled() -> AppError {
    AppError::cancelled("the recognition was stopped")
}

impl Inner {
    fn state(&self) -> MutexGuard<'_, ModelState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn start_running(&self) -> Running<'_> {
        let token = CancelToken::new();
        self.state().running.push(token.clone());
        Running {
            engine: self,
            token,
        }
    }

    fn provider_id(&self) -> ProviderId {
        ProviderId::new(format!("{}/{}", self.model.engine, self.model.id))
    }

    /// The model is freed and the store told only after the state lock is released.
    fn unload_if(&self, wanted: impl FnOnce(&ModelState) -> bool) -> AppResult<bool> {
        let taken = {
            let mut state = self.state();
            let busy = state.in_use > 0 || state.loading;
            if !wanted(&state) || (state.model.is_none() && !busy) {
                return Ok(false);
            }
            if busy {
                return Err(AppError::already_exists(format!(
                    "model {} is in use and cannot be unloaded now",
                    self.model.id
                )));
            }
            state.model.take()
        };
        drop(taken);
        self.store.mark_loaded(&self.model.id, false)?;
        Ok(true)
    }

    /// Loads (or reloads an unusable model) when needed, waiting if another thread is loading.
    /// Returns the load time when this call loaded it.
    fn hold_model(&self, cancel: &CancelToken) -> AppResult<(InUse<'_>, Option<u64>)> {
        // Declared first so that it is freed after the state is unlocked.
        let mut discarded = None;
        let mut state = self.state();
        loop {
            if cancel.is_cancelled() {
                return Err(cancelled());
            }
            if let Some(model) = &state.model {
                if model.is_usable() {
                    let model = Arc::clone(model);
                    state.in_use += 1;
                    return Ok((
                        InUse {
                            engine: self,
                            model,
                        },
                        None,
                    ));
                }
                discarded = state.model.take();
            }
            if !state.loading {
                break;
            }
            state = self
                .load_finished
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
        state.loading = true;
        drop(state);
        let _ends = LoadEnds(self);
        self.let_go(discarded);

        let started = self.clock.monotonic_ms();
        let loaded = self.load_model(cancel);
        let finished = self.clock.monotonic_ms();

        let mut state = self.state();
        loaded.map(|model| {
            state.model = Some(Arc::clone(&model));
            state.in_use += 1;
            state.last_used_ms = finished;
            (
                InUse {
                    engine: self,
                    model,
                },
                Some(finished.saturating_sub(started)),
            )
        })
    }

    fn discard_if_unusable(&self, model: &Arc<dyn SpeechModel>) {
        if model.is_usable() {
            return;
        }
        let taken = {
            let mut state = self.state();
            match &state.model {
                Some(current) if Arc::ptr_eq(current, model) => state.model.take(),
                _ => None,
            }
        };
        self.let_go(taken);
    }

    fn let_go(&self, taken: Option<Arc<dyn SpeechModel>>) {
        if taken.is_some() {
            drop(taken);
            // The caller gets the recognition's outcome; the store is told again at next load.
            self.store.mark_loaded(&self.model.id, false).ok();
        }
    }

    fn load_model(&self, cancel: &CancelToken) -> AppResult<Arc<dyn SpeechModel>> {
        let dir = self
            .store
            .local_path(&self.model.id)?
            .ok_or_else(|| AppError::model(format!("model {} is not installed", self.model.id)))?;
        let model: Arc<dyn SpeechModel> =
            Arc::from(self.backend.load(&self.model, &dir, &self.tuning, cancel)?);
        self.store.mark_loaded(&self.model.id, true)?;
        Ok(model)
    }

    pub(crate) fn check(&self, opts: &AsrOptions, duration_secs: Option<u32>) -> AppResult<()> {
        if let Some(wanted) = &opts.model_id
            && wanted != &self.model.id
        {
            return Err(AppError::validation(format!(
                "this engine recognises with model {}, not {wanted}",
                self.model.id
            )));
        }
        let violations = self.caps.check_options(opts, duration_secs);
        if violations.is_empty() {
            Ok(())
        } else {
            Err(AppError::unsupported(format!(
                "model {} cannot do what was asked: {violations:?}",
                self.model.id
            )))
        }
    }

    pub(crate) fn check_length(&self, samples: usize) -> AppResult<()> {
        let rate = self.caps.sample_rate_hz as usize;
        match self.caps.max_duration_secs {
            Some(max_secs) if samples > (max_secs as usize).saturating_mul(rate) => {
                Err(AppError::validation(format!(
                    "this piece of audio is longer than the {max_secs} s model {} takes at once; \
                     cut longer recordings into pieces",
                    self.model.id
                )))
            }
            _ => Ok(()),
        }
    }

    /// `samples` are mono at the model's sample rate.
    pub(crate) fn recognise(&self, samples: &[f32], opts: &AsrOptions) -> AppResult<Transcript> {
        let rate = u64::from(self.caps.sample_rate_hz);
        let duration_ms = (samples.len() as u64).saturating_mul(1_000) / rate;
        self.check(opts, None)?;
        self.check_length(samples.len())?;

        let mut timings = StageTimings::default();
        let number = self.transcripts.fetch_add(1, Ordering::Relaxed);
        let id = TranscriptId::new(format!(
            "{}-{}-{number}",
            self.model.id,
            self.clock.now_ms()
        ));
        let text = if samples.is_empty() {
            String::new()
        } else {
            let running = self.start_running();
            let mut tried = false;
            loop {
                let (held, load_ms) = self.hold_model(&running.token)?;
                if let Some(load_ms) = load_ms {
                    timings.record(STAGE_MODEL_LOAD, load_ms);
                }
                let started = self.clock.monotonic_ms();
                let outcome =
                    held.model
                        .recognise(samples, self.caps.sample_rate_hz, opts, &running.token);
                let model = Arc::clone(&held.model);
                drop(held);
                self.discard_if_unusable(&model);
                // A model that stopped working (e.g. its worker was killed by another
                // recognition's cancel) is reloaded and tried once more.
                if outcome.is_err() && !tried && !model.is_usable() && !running.token.is_cancelled()
                {
                    tried = true;
                    continue;
                }
                let text = outcome?;
                timings.record(
                    STAGE_RECOGNITION,
                    self.clock.monotonic_ms().saturating_sub(started),
                );
                break text.trim().to_owned();
            }
        };

        let mut transcript = if text.is_empty() {
            Transcript::new(id, text, opts.languages.clone())
        } else {
            let segment = TranscriptSegment::new(text, 0, duration_ms)?;
            Transcript::from_segments(id, vec![segment], opts.languages.clone())
        };
        transcript.audio_duration_ms = duration_ms;
        transcript.timings = timings;
        transcript.add_route(RequestRoute::local(
            STAGE_RECOGNITION,
            self.provider_id(),
            Some(self.model.id.clone()),
        ));
        Ok(transcript)
    }
}

impl AsrEngine for LocalAsrEngine {
    fn id(&self) -> ProviderId {
        self.inner.provider_id()
    }

    fn capabilities(&self) -> AsrCaps {
        self.inner.caps.clone()
    }

    /// Cancels unfinished work only; work requested afterwards is not affected.
    fn interrupt(&self) {
        let running = std::mem::take(&mut self.inner.state().running);
        for token in running {
            token.cancel();
        }
    }

    fn start(&self, opts: AsrOptions) -> AppResult<Box<dyn AsrSession>> {
        self.inner.check(&opts, None)?;
        Ok(Box::new(BufferedSession::new(
            Arc::clone(&self.inner),
            opts,
        )))
    }
}
