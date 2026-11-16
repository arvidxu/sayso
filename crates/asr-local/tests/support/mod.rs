#![allow(dead_code)]

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use asr_local::{LocalAsrEngine, ModelBackend, RecognitionTuning, SpeechModel};
use dictation_api::ModelStore;
use dictation_api::fakes::{FakeClock, FakeModelStore};
use dictation_types::{
    AppError, AppResult, AsrOptions, CancelToken, Language, Languages, ModelDescriptor, ModelFile,
    ModelId, ModelKind, PlatformTarget, ProviderId,
};

mod watched_store;

pub use watched_store::WatchedStore;

pub const ENGINE: &str = "engine-x";

#[derive(Debug, Clone)]
pub struct LoadCall {
    pub model_id: ModelId,
    pub dir: PathBuf,
    pub tuning: RecognitionTuning,
}

#[derive(Debug, Clone)]
pub struct RecogniseCall {
    pub samples: Vec<f32>,
    pub sample_rate_hz: u32,
    pub opts: AsrOptions,
}

/// Holds callers back while closed, and counts how many are waiting.
#[derive(Default)]
struct Gate {
    state: Mutex<(bool, usize)>,
    changed: Condvar,
}

impl Gate {
    fn set_closed(&self, closed: bool) {
        locked(&self.state).0 = closed;
        self.changed.notify_all();
    }

    fn pass(&self) {
        let mut state = locked(&self.state);
        state.1 += 1;
        while state.0 {
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
        state.1 -= 1;
    }

    fn waiting(&self) -> usize {
        locked(&self.state).1
    }
}

/// How long to wait for another thread before failing.
pub const PATIENCE: Duration = Duration::from_secs(10);

/// Panics after [`PATIENCE`].
pub fn wait_until(what: &str, done: impl Fn() -> bool) {
    let started = Instant::now();
    while !done() {
        assert!(started.elapsed() < PATIENCE, "gave up waiting until {what}");
        thread::sleep(Duration::from_millis(2));
    }
}

/// Panics after [`PATIENCE`], so a hang fails the test instead of stalling it.
pub fn promptly<T: Send + 'static>(what: &str, work: impl FnOnce() -> T + Send + 'static) -> T {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(work());
    });
    receiver
        .recv_timeout(PATIENCE)
        .unwrap_or_else(|_| panic!("{what} did not return"))
}

struct Script {
    engine_id: String,
    clock: FakeClock,
    loads: Mutex<Vec<LoadCall>>,
    calls: Mutex<Vec<RecogniseCall>>,
    alive: AtomicUsize,
    check_error: Mutex<Option<AppError>>,
    load_errors: Mutex<VecDeque<AppError>>,
    answers: Mutex<VecDeque<AppResult<String>>>,
    usual_answer: Mutex<String>,
    load_takes_ms: AtomicU64,
    recognise_takes_ms: AtomicU64,
    load_gate: Gate,
    recognise_gate: Gate,
}

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A fake backend driven by the test; clones share one script.
#[derive(Clone)]
pub struct ScriptedBackend {
    script: Arc<Script>,
}

impl ScriptedBackend {
    pub fn new(engine_id: &str, clock: FakeClock) -> Self {
        Self {
            script: Arc::new(Script {
                engine_id: engine_id.to_owned(),
                clock,
                loads: Mutex::new(Vec::new()),
                calls: Mutex::new(Vec::new()),
                alive: AtomicUsize::new(0),
                check_error: Mutex::new(None),
                load_errors: Mutex::new(VecDeque::new()),
                answers: Mutex::new(VecDeque::new()),
                usual_answer: Mutex::new("recognised words".to_owned()),
                load_takes_ms: AtomicU64::new(0),
                recognise_takes_ms: AtomicU64::new(0),
                load_gate: Gate::default(),
                recognise_gate: Gate::default(),
            }),
        }
    }

    pub fn refuse_check(&self, error: AppError) {
        *locked(&self.script.check_error) = Some(error);
    }

    pub fn fail_next_load(&self, error: AppError) {
        locked(&self.script.load_errors).push_back(error);
    }

    /// Used once, then back to the default answer.
    pub fn answer_next(&self, text: &str) {
        locked(&self.script.answers).push_back(Ok(text.to_owned()));
    }

    pub fn fail_next_recognition(&self, error: AppError) {
        locked(&self.script.answers).push_back(Err(error));
    }

    /// Test-clock time, not real time.
    pub fn loading_takes_ms(&self, ms: u64) {
        self.script.load_takes_ms.store(ms, Ordering::SeqCst);
    }

    /// Test-clock time, not real time.
    pub fn recognising_takes_ms(&self, ms: u64) {
        self.script.recognise_takes_ms.store(ms, Ordering::SeqCst);
    }

    /// Loads wait, after being recorded, until [`Self::release_loads`].
    pub fn hold_loads(&self) {
        self.script.load_gate.set_closed(true);
    }

    pub fn release_loads(&self) {
        self.script.load_gate.set_closed(false);
    }

    pub fn loads_held(&self) -> usize {
        self.script.load_gate.waiting()
    }

    /// Recognitions wait, after being recorded, until [`Self::release_recognitions`].
    pub fn hold_recognitions(&self) {
        self.script.recognise_gate.set_closed(true);
    }

    pub fn release_recognitions(&self) {
        self.script.recognise_gate.set_closed(false);
    }

    pub fn recognitions_held(&self) -> usize {
        self.script.recognise_gate.waiting()
    }

    pub fn loads(&self) -> Vec<LoadCall> {
        locked(&self.script.loads).clone()
    }

    pub fn load_count(&self) -> usize {
        locked(&self.script.loads).len()
    }

    pub fn calls(&self) -> Vec<RecogniseCall> {
        locked(&self.script.calls).clone()
    }

    pub fn call_count(&self) -> usize {
        locked(&self.script.calls).len()
    }

    pub fn models_alive(&self) -> usize {
        self.script.alive.load(Ordering::SeqCst)
    }
}

impl ModelBackend for ScriptedBackend {
    fn engine_id(&self) -> ProviderId {
        ProviderId::new(self.script.engine_id.clone())
    }

    fn check(&self, _model: &ModelDescriptor) -> AppResult<()> {
        match locked(&self.script.check_error).clone() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn load(
        &self,
        model: &ModelDescriptor,
        dir: &Path,
        tuning: &RecognitionTuning,
        _cancel: &CancelToken,
    ) -> AppResult<Box<dyn SpeechModel>> {
        locked(&self.script.loads).push(LoadCall {
            model_id: model.id.clone(),
            dir: dir.to_path_buf(),
            tuning: tuning.clone(),
        });
        self.script.load_gate.pass();
        self.script
            .clock
            .advance_ms(self.script.load_takes_ms.load(Ordering::SeqCst));
        if let Some(error) = locked(&self.script.load_errors).pop_front() {
            return Err(error);
        }
        self.script.alive.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(ScriptedModel {
            script: Arc::clone(&self.script),
            usable: Arc::new(AtomicBool::new(true)),
        }))
    }
}

/// Like a model in a worker process, a cancel makes it unusable at once.
struct ScriptedModel {
    script: Arc<Script>,
    usable: Arc<AtomicBool>,
}

impl SpeechModel for ScriptedModel {
    fn recognise(
        &self,
        samples: &[f32],
        sample_rate_hz: u32,
        opts: &AsrOptions,
        cancel: &CancelToken,
    ) -> AppResult<String> {
        let usable = Arc::clone(&self.usable);
        let _watch = cancel.on_cancel(move || usable.store(false, Ordering::SeqCst));
        locked(&self.script.calls).push(RecogniseCall {
            samples: samples.to_vec(),
            sample_rate_hz,
            opts: opts.clone(),
        });
        self.script.recognise_gate.pass();
        self.script
            .clock
            .advance_ms(self.script.recognise_takes_ms.load(Ordering::SeqCst));
        if cancel.is_cancelled() {
            return Err(AppError::cancelled("the recognition was stopped"));
        }
        if !self.is_usable() {
            return Err(AppError::asr(
                "the model was stopped by another recognition",
            ));
        }
        match locked(&self.script.answers).pop_front() {
            Some(answer) => answer,
            None => Ok(locked(&self.script.usual_answer).clone()),
        }
    }

    fn is_usable(&self) -> bool {
        self.usable.load(Ordering::SeqCst)
    }
}

impl Drop for ScriptedModel {
    fn drop(&mut self) {
        self.script.alive.fetch_sub(1, Ordering::SeqCst);
    }
}

pub fn entry_of_kind(
    kind: ModelKind,
    id: &str,
    engine: &str,
    languages: &[&str],
    settings: &[(&str, &str)],
) -> ModelDescriptor {
    let descriptor = ModelDescriptor {
        id: ModelId::new(id),
        name: format!("Test model {id}"),
        kind,
        engine: ProviderId::new(engine),
        group: None,
        files: vec![ModelFile {
            path: "weights.bin".to_owned(),
            size_bytes: 64,
            sha256: "0123456789abcdef".repeat(4),
            download_urls: vec!["https://example.invalid/weights.bin".to_owned()],
            unpack: None,
        }],
        provided_by_system: false,
        named_paths: Vec::new(),
        engine_settings: settings
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect(),
        supported_platforms: PlatformTarget::ALL.to_vec(),
        supported_languages: languages.iter().map(|tag| language(tag)).collect(),
    };
    descriptor.validate().expect("a valid test entry");
    descriptor
}

pub fn speech_entry(
    id: &str,
    engine: &str,
    languages: &[&str],
    settings: &[(&str, &str)],
) -> ModelDescriptor {
    entry_of_kind(ModelKind::Asr, id, engine, languages, settings)
}

/// Replaces, adds, or (when `value` is `None`) removes one engine setting.
pub fn with_setting(
    mut model: ModelDescriptor,
    name: &str,
    value: Option<&str>,
) -> ModelDescriptor {
    model.engine_settings.retain(|(n, _)| n != name);
    if let Some(value) = value {
        model
            .engine_settings
            .push((name.to_owned(), value.to_owned()));
    }
    model
}

pub fn language(tag: &str) -> Language {
    Language::parse(tag).expect("a valid language tag")
}

/// `spec` is like `sw` or `ko+fi+ta`.
pub fn options(spec: &str) -> AsrOptions {
    AsrOptions::new(Languages::parse(spec).expect("valid languages"))
}

pub fn tuning() -> RecognitionTuning {
    RecognitionTuning {
        thread_share: 0.5,
        execution_provider: "test-processor".to_owned(),
        load_timeout_ms: 60_000,
        worker_parent_check_ms: 1_000,
    }
}

pub struct Fixture {
    pub backend: ScriptedBackend,
    pub store: FakeModelStore,
    pub watched: WatchedStore,
    pub clock: FakeClock,
    pub model: ModelDescriptor,
    pub engine: LocalAsrEngine,
}

impl Fixture {
    /// In the catalog but not installed.
    pub fn not_installed(model: ModelDescriptor) -> Self {
        let clock = FakeClock::new(1_000_000);
        let backend = ScriptedBackend::new(model.engine.as_str(), clock.clone());
        let store = FakeModelStore::new(vec![model.clone()]);
        let watched = WatchedStore::new(store.clone());
        let engine = LocalAsrEngine::new(
            Arc::new(backend.clone()),
            Arc::new(watched.clone()),
            Arc::new(clock.clone()),
            model.clone(),
            tuning(),
        )
        .expect("an engine for a valid entry");
        Self {
            backend,
            store,
            watched,
            clock,
            model,
            engine,
        }
    }

    pub fn installed(model: ModelDescriptor) -> Self {
        let fixture = Self::not_installed(model);
        fixture.install();
        fixture
    }

    pub fn folder(&self) -> PathBuf {
        PathBuf::from(format!("installed-models/{}", self.model.id))
    }

    pub fn install(&self) {
        self.store
            .import_local(&self.model.id, &self.folder())
            .expect("the store accepts the model");
    }

    /// `true` for loaded, `false` for unloaded, in order.
    pub fn marks(&self) -> Vec<bool> {
        self.watched.marks()
    }

    pub fn refuse_next_mark_loaded(&self, error: AppError) {
        self.watched.refuse_next_mark_loaded(error);
    }

    pub fn store_says_loaded(&self) -> bool {
        self.store
            .get_status(&self.model.id)
            .expect("the store answers")
            .expect("the model is in the catalog")
            .state
            .is_loaded()
    }
}

/// All values differ, so order and content can be checked.
pub fn ramp(count: usize) -> Vec<f32> {
    (0..count)
        .map(|i| (i % 1_000) as f32 / 1_000.0 - 0.5)
        .collect()
}
