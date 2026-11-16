//! Measures how well an installed model recognises a set of recordings whose text is known.
//!
//! ```text
//! cargo run --release -p engine-worker --features sherpa-onnx --example evaluate -- \
//!     [--catalog <file>] [--worker <worker program>] [--tell-language yes|no] \
//!     [--cut at-pauses] \
//!     <model id> <models folder> \
//!     <manifest.tsv> [language ...]
//! ```
//!
//! The manifest has one tab-separated recording per line: language (`fr+en` when the speaker
//! switches), file relative to the manifest, and the reference text. `--details <file>`
//! writes per-recording results; `evaluate --rescore <file>` rescores such a file. Failed
//! recordings are reported and skipped. Read word accuracy for languages written with
//! spaces, character accuracy for those without.

mod support;

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use asr_local::error_rate::{ErrorCount, character_errors, word_errors};
use dictation_types::{AppError, AppResult, AsrOptions, Languages, ModelId};
use engine_worker::WorkerCommand;
use support::{Recogniser, take_cut, take_option};

/// Third field of a details line for a failed recording; the last field then says why.
const FAILED: &str = "failed";

const USAGE: &str = "usage: evaluate [--catalog <file>] [--worker <worker program>] \
     [--tell-language yes|no] [--cut at-pauses] [--details <file>] <model id> \
     <models folder> <manifest.tsv> [language ...]";

#[derive(Default)]
struct Tally {
    recordings: usize,
    /// Not included in the counts.
    failed: usize,
    words: ErrorCount,
    characters: ErrorCount,
    audio_secs: f32,
    took_secs: f32,
}

fn main() -> ExitCode {
    match evaluate(env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("failed: {err}");
            ExitCode::FAILURE
        }
    }
}

fn evaluate(mut args: Vec<String>) -> AppResult<()> {
    if let Some(listing) = take_option(&mut args, "--rescore")? {
        return rescore(Path::new(&listing));
    }
    let catalog = take_option(&mut args, "--catalog")?.map(PathBuf::from);
    let worker = take_option(&mut args, "--worker")?.map(WorkerCommand::new);
    let tell = match take_option(&mut args, "--tell-language")?.as_deref() {
        None | Some("yes") => true,
        Some("no") => false,
        Some(other) => {
            return Err(AppError::validation(format!(
                "--tell-language takes yes or no, not '{other}'"
            )));
        }
    };
    let at_pauses = take_cut(&mut args)?;
    let details = take_option(&mut args, "--details")?.map(PathBuf::from);
    if args.len() < 3 {
        return Err(AppError::validation(USAGE));
    }
    let (id, folder) = (ModelId::new(&args[0]), PathBuf::from(&args[1]));
    let mut recogniser = Recogniser::open(worker, catalog.as_deref(), &id, folder)?;
    recogniser.at_pauses = at_pauses;
    run(
        recogniser,
        tell,
        details.as_deref(),
        &id,
        Path::new(&args[2]),
        &args[3..],
    )
}

/// Clamped at 0 even when the model inserted more than was said.
fn correct(count: &ErrorCount) -> String {
    count.rate().map_or_else(
        || "-".to_owned(),
        |rate| format!("{:.1}%", (1.0 - rate).max(0.0) * 100.0),
    )
}

fn print_table(tallies: &BTreeMap<String, Tally>) {
    println!("language  recordings  failed  correct words  correct characters  audio (s)  speed");
    for (language, tally) in tallies {
        println!(
            "{language:<8}  {:>10}  {:>6}  {:>13}  {:>18}  {:>9.0}  {:>4.1}x",
            tally.recordings,
            tally.failed,
            correct(&tally.words),
            correct(&tally.characters),
            tally.audio_secs,
            tally.audio_secs / tally.took_secs.max(f32::EPSILON),
        );
    }
}

/// Useful when the scoring method has changed.
fn rescore(listing: &Path) -> AppResult<()> {
    let text = fs::read_to_string(listing)
        .map_err(|err| AppError::storage(format!("{}: {err}", listing.display())))?;
    let mut tallies: BTreeMap<String, Tally> = BTreeMap::new();
    let mut skipped = 0;
    for line in text.lines() {
        let fields: Vec<&str> = line.split('\t').collect();
        let [language, _, outcome, said, heard] = fields[..] else {
            skipped += 1;
            continue;
        };
        let tally = tallies.entry(language.to_owned()).or_default();
        if outcome == FAILED {
            tally.failed += 1;
            continue;
        }
        tally.recordings += 1;
        tally.words = tally.words.plus(&word_errors(said, heard));
        tally.characters = tally.characters.plus(&character_errors(said, heard));
    }
    print_table(&tallies);
    if skipped > 0 {
        println!("{skipped} lines of the listing are not well formed and were not scored");
    }
    Ok(())
}

fn run(
    mut recogniser: Recogniser,
    tell_language: bool,
    details: Option<&Path>,
    id: &ModelId,
    manifest: &Path,
    only: &[String],
) -> AppResult<()> {
    recogniser.engine.load()?;
    let every_language = Languages::new(recogniser.model.supported_languages.clone())?;
    let listing = fs::read_to_string(manifest)
        .map_err(|err| AppError::storage(format!("{}: {err}", manifest.display())))?;
    let base = manifest.parent().unwrap_or_else(|| Path::new(""));

    let mut tallies: BTreeMap<String, Tally> = BTreeMap::new();
    // One line per recording: language, file, wrong and total characters, said, heard; or,
    // for a recording that failed, language, file, `failed`, said, and why it failed.
    let mut listing_out = String::new();
    // Recordings in languages the model does not know, or cannot take together.
    let mut left_out: BTreeMap<String, usize> = BTreeMap::new();
    for line in listing.lines().filter(|line| !line.trim().is_empty()) {
        let mut fields = line.splitn(3, '\t');
        let (Some(language), Some(file), Some(said)) =
            (fields.next(), fields.next(), fields.next())
        else {
            return Err(AppError::validation(format!(
                "not a manifest line: '{line}'"
            )));
        };
        if !only.is_empty() && !only.iter().any(|wanted| wanted == language) {
            continue;
        }
        // Tabs and line ends inside the texts would split the listing's fields.
        let flat = |text: &str| text.replace(['\t', '\n', '\r'], " ");
        let options = match Languages::parse(language) {
            Ok(_) if !tell_language => AsrOptions::new(every_language.clone()),
            Ok(languages) => AsrOptions::new(languages),
            Err(err) => {
                eprintln!("{file}: the manifest's language '{language}' cannot be read: {err}");
                tallies.entry(language.to_owned()).or_default().failed += 1;
                listing_out.push_str(&format!(
                    "{language}\t{file}\t{FAILED}\t{}\t{}\n",
                    flat(said),
                    flat(&err.to_string())
                ));
                continue;
            }
        };
        if !recogniser.caps.supports_languages(&options.languages) {
            *left_out.entry(language.to_owned()).or_default() += 1;
            continue;
        }
        let started = Instant::now();
        let recognised = recogniser.recognise(&base.join(file), &options);
        let tally = tallies.entry(language.to_owned()).or_default();
        let heard = match recognised {
            Ok(heard) => heard,
            Err(err) => {
                eprintln!("{file}: failed: {err}");
                tally.failed += 1;
                listing_out.push_str(&format!(
                    "{language}\t{file}\t{FAILED}\t{}\t{}\n",
                    flat(said),
                    flat(&err.to_string())
                ));
                continue;
            }
        };
        tally.recordings += 1;
        tally.words = tally.words.plus(&word_errors(said, &heard.text));
        let characters = character_errors(said, &heard.text);
        tally.characters = tally.characters.plus(&characters);
        listing_out.push_str(&format!(
            "{language}\t{file}\t{}/{}\t{}\t{}\n",
            characters.errors,
            characters.reference,
            flat(said),
            flat(&heard.text)
        ));
        tally.audio_secs += heard.audio_secs;
        tally.took_secs += started.elapsed().as_secs_f32();
    }

    println!(
        "model {id}, language {}",
        if tell_language { "told" } else { "not told" }
    );
    print_table(&tallies);
    if let Some(path) = details {
        fs::write(path, listing_out)
            .map_err(|err| AppError::storage(format!("{}: {err}", path.display())))?;
    }
    if !left_out.is_empty() {
        let list: Vec<String> = left_out
            .iter()
            .map(|(language, recordings)| format!("{language} ({recordings})"))
            .collect();
        println!(
            "left out, the model does not take them: {}",
            list.join(", ")
        );
    }
    Ok(())
}
