//! Test engine that can be told to kill its process, hang, be slow, or print on the reply stream.

use std::io::stdout;
use std::path::Path;
use std::process::abort;
use std::thread;

use asr_local::{ModelBackend, RecognitionTuning, SpeechModel};
use dictation_types::{AppError, AppResult, AsrOptions, CancelToken, ModelDescriptor, ProviderId};

use super::{ENGINE, HANG, ON_LOAD, PATIENCE, SLOW, SLOW_FOR, STARTS_FILE, STOP_PROCESS};

pub struct TestEngine;

/// Adds a line to the file the tests watch, once the work has reached the engine.
fn note_start() {
    if let Some(path) = std::env::var_os(STARTS_FILE) {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path);
        if let Ok(mut file) = file {
            let _ = std::io::Write::write_all(&mut file, b"begun\n");
        }
    }
}

struct TestModel {
    loaded_from: String,
    tuning: RecognitionTuning,
}

impl ModelBackend for TestEngine {
    fn engine_id(&self) -> ProviderId {
        ProviderId::new(ENGINE)
    }

    fn check(&self, model: &ModelDescriptor) -> AppResult<()> {
        match model.engine_setting(ON_LOAD) {
            Some(_) => Ok(()),
            None => Err(AppError::validation("the entry does not say what to do")),
        }
    }

    fn load(
        &self,
        model: &ModelDescriptor,
        dir: &Path,
        tuning: &RecognitionTuning,
        _cancel: &CancelToken,
    ) -> AppResult<Box<dyn SpeechModel>> {
        note_start();
        match model.engine_setting(ON_LOAD) {
            Some("stop-process") => abort(),
            Some("hang") => loop {
                thread::sleep(PATIENCE);
            },
            Some("fail") => return Err(AppError::checksum_mismatch("the weights are damaged")),
            Some("chatter") => {
                println!("{{\"text\":\"not a reply\"}}");
                print!("printed without a line end");
                std::io::Write::flush(&mut stdout()).ok();
            }
            _ => {}
        }
        Ok(Box::new(TestModel {
            loaded_from: dir.display().to_string(),
            tuning: tuning.clone(),
        }))
    }
}

impl SpeechModel for TestModel {
    fn recognise(
        &self,
        samples: &[f32],
        sample_rate_hz: u32,
        opts: &AsrOptions,
        _cancel: &CancelToken,
    ) -> AppResult<String> {
        note_start();
        match samples.first() {
            Some(first) if *first > STOP_PROCESS => abort(),
            Some(first) if *first < HANG => loop {
                thread::sleep(PATIENCE);
            },
            Some(first) if *first > SLOW => thread::sleep(SLOW_FOR),
            _ => {}
        }
        let sum: f64 = samples.iter().map(|s| f64::from(*s)).sum();
        Ok(format!(
            "pid={} n={} rate={sample_rate_hz} sum={sum:.4} langs={} from={} share={} on={} \
             hot={}",
            std::process::id(),
            samples.len(),
            opts.languages.code(),
            self.loaded_from,
            self.tuning.thread_share,
            self.tuning.execution_provider,
            opts.hotwords
                .iter()
                .map(|term| term.canonical.as_str())
                .collect::<Vec<_>>()
                .join("|"),
        ))
    }
}
