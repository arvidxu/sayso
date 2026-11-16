use super::*;

pub fn entry(id: &str, on_load: &str) -> ModelDescriptor {
    let model = ModelDescriptor {
        id: ModelId::new(id),
        name: format!("Test model {id}"),
        kind: ModelKind::Asr,
        engine: ProviderId::new(ENGINE),
        group: Some("tests".to_owned()),
        files: vec![ModelFile {
            path: "weights.bin".to_owned(),
            size_bytes: 64,
            sha256: "0123456789abcdef".repeat(4),
            download_urls: vec!["https://example.invalid/weights.bin".to_owned()],
            unpack: None,
        }],
        provided_by_system: false,
        named_paths: vec![("weights".to_owned(), "weights.bin".to_owned())],
        engine_settings: vec![
            (ON_LOAD.to_owned(), on_load.to_owned()),
            ("sampleRateHz".to_owned(), "16000".to_owned()),
            ("languageMixing".to_owned(), "mixed".to_owned()),
            ("supportsHotwords".to_owned(), "true".to_owned()),
        ],
        supported_platforms: PlatformTarget::ALL.to_vec(),
        supported_languages: ["sw", "is", "ta"]
            .iter()
            .map(|tag| Language::parse(tag).unwrap())
            .collect(),
    };
    model.validate().unwrap();
    model
}

pub fn tuning() -> RecognitionTuning {
    RecognitionTuning {
        thread_share: 0.25,
        execution_provider: "test-processor".to_owned(),
        load_timeout_ms: 60_000,
        worker_parent_check_ms: 1_000,
    }
}

pub fn worker_backend() -> Arc<WorkerBackend> {
    let me = env::current_exe().expect("this test's own program");
    Arc::new(WorkerBackend::new(
        Arc::new(TestEngine),
        WorkerCommand::new(me),
    ))
}

pub fn folder(model: &ModelDescriptor) -> PathBuf {
    PathBuf::from(format!("installed-models/{}", model.id))
}

pub fn engine_for(model: &ModelDescriptor) -> LocalAsrEngine {
    engine_with(model, tuning())
}

pub fn engine_with(model: &ModelDescriptor, tuning: RecognitionTuning) -> LocalAsrEngine {
    let store = FakeModelStore::new(vec![model.clone()]);
    store.import_local(&model.id, &folder(model)).unwrap();
    LocalAsrEngine::new(
        worker_backend(),
        Arc::new(store),
        Arc::new(FakeClock::new(1_000_000)),
        model.clone(),
        tuning,
    )
    .unwrap()
}

pub fn options(spec: &str) -> AsrOptions {
    AsrOptions::new(Languages::parse(spec).unwrap())
}

pub fn field<'a>(text: &'a str, name: &str) -> &'a str {
    text.split(' ')
        .find_map(|part| part.strip_prefix(&format!("{name}=")))
        .unwrap_or_else(|| panic!("no {name} in '{text}'"))
}

pub fn sound(first: f32) -> Vec<f32> {
    let mut samples: Vec<f32> = (0..4_000).map(|i| (i % 200) as f32 / 400.0).collect();
    samples[0] = first;
    samples
}

/// Asks the system's own tools.
pub fn is_running(pid: &str) -> bool {
    #[cfg(unix)]
    return std::process::Command::new("kill")
        .args(["-0", pid])
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    #[cfg(windows)]
    return std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH"])
        .output()
        .is_ok_and(|out| String::from_utf8_lossy(&out.stdout).contains(pid));
}

/// How many loads and recognitions have begun in any worker so far.
pub fn starts() -> usize {
    env::var_os(STARTS_FILE)
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map_or(0, |text| text.lines().count())
}

/// Waits until more than `seen` loads and recognitions have begun.
pub fn wait_for_start(seen: usize) {
    let started = Instant::now();
    while starts() <= seen {
        assert!(started.elapsed() < PATIENCE, "nothing began");
        thread::sleep(Duration::from_millis(5));
    }
}
