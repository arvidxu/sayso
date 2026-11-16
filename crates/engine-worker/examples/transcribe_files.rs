//! Recognises WAV files with an installed model.
//!
//! ```text
//! cargo run --release -p engine-worker --features sherpa-onnx --example transcribe_files -- \
//!     [--catalog <file>] [--worker <worker program>] [--languages <codes>] \
//!     [--hotwords <a,b,...>] \
//!     [--cut at-pauses] \
//!     <model id> <models folder> <file.wav>...
//! ```
//!
//! Without `--worker` the engine runs in-process. `--languages` is like `ja` or `fr+en`.
//! `--cut at-pauses` recognises each stretch of speech between pauses separately.

mod support;

use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use dictation_api::AsrEngine;
use dictation_types::{
    AppError, AppResult, AsrOptions, Languages, ModelId, Term, TermId, TermKind,
};
use engine_worker::WorkerCommand;
use support::{Recogniser, take_cut, take_option};

const USAGE: &str = "usage: transcribe_files [--catalog <file>] [--worker <worker program>] \
     [--languages <codes>] [--hotwords <a,b,...>] [--cut at-pauses] \
     <model id> <models folder> <file.wav>...";

fn main() -> ExitCode {
    match transcribe(env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("failed: {err}");
            ExitCode::FAILURE
        }
    }
}

fn transcribe(mut args: Vec<String>) -> AppResult<()> {
    let catalog = take_option(&mut args, "--catalog")?.map(PathBuf::from);
    let worker = take_option(&mut args, "--worker")?.map(WorkerCommand::new);
    let languages = take_option(&mut args, "--languages")?;
    let hotwords = take_option(&mut args, "--hotwords")?;
    let at_pauses = take_cut(&mut args)?;
    if args.len() < 3 {
        return Err(AppError::validation(USAGE));
    }
    let (id, folder) = (ModelId::new(&args[0]), PathBuf::from(&args[1]));
    let hotwords: Vec<&str> = hotwords.iter().flat_map(|words| words.split(',')).collect();
    let mut recogniser = Recogniser::open(worker, catalog.as_deref(), &id, folder)?;
    recogniser.at_pauses = at_pauses;
    recogniser.show_pieces = at_pauses;
    run(recogniser, languages.as_deref(), &hotwords, &args[2..])
}

fn run(
    mut recogniser: Recogniser,
    languages: Option<&str>,
    hotwords: &[&str],
    files: &[String],
) -> AppResult<()> {
    println!(
        "engine {}  threads {}  takes {} Hz, at most {:?} s at once",
        recogniser.engine.id(),
        recogniser.threads,
        recogniser.caps.sample_rate_hz,
        recogniser.caps.max_duration_secs
    );
    let started = Instant::now();
    recogniser.engine.load()?;
    println!("model loaded in {:.2} s", started.elapsed().as_secs_f32());

    let mut options = AsrOptions::new(match languages {
        Some(spec) => Languages::parse(spec)?,
        None => Languages::new(recogniser.model.supported_languages.clone())?,
    });
    for (number, word) in hotwords.iter().enumerate() {
        options.hotwords.push(Term::new(
            TermId::new(number.to_string()),
            *word,
            TermKind::default(),
            options.languages.primary().clone(),
        )?);
    }
    for file in files {
        let name = Path::new(file)
            .file_name()
            .map_or_else(|| file.clone(), |name| name.to_string_lossy().into_owned());
        let started = Instant::now();
        let heard = recogniser.recognise(Path::new(file), &options)?;
        let took = started.elapsed().as_secs_f32();
        println!(
            "{name}: {:.1} s of audio in {} piece(s), {took:.2} s ({:.1}x real time)\n  {}",
            heard.audio_secs,
            heard.pieces,
            heard.audio_secs / took,
            heard.text
        );
    }
    Ok(())
}
