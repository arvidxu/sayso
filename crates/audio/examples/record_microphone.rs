//! Records from a real microphone, saves a WAV file and prints where speech was found.
//!
//! ```text
//! cargo run -p audio --example record_microphone -- <seconds> <output.wav> [microphone]
//! ```

use std::env;
use std::fs;
use std::io::{Write, stdout};
use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use audio::{AudioConfig, FrameScorer, MicrophoneSource, NeuralSpeechScorer, VoiceDetector};
use dictation_api::AudioSource;
use dictation_types::{AppError, AppResult, AudioCaptureConfig, AudioChunk};

const EXAMPLE_CHUNK_MS: u32 = 20;

const LEVEL_EVERY: Duration = Duration::from_millis(200);

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let (Some(seconds), Some(output)) = (
        args.first().and_then(|s| s.parse::<u64>().ok()),
        args.get(1),
    ) else {
        eprintln!("usage: record_microphone <seconds> <output.wav> [microphone]");
        return ExitCode::FAILURE;
    };
    match run(
        Duration::from_secs(seconds),
        Path::new(output),
        args.get(2).map(String::as_str),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("failed: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run(length: Duration, output: &Path, wanted: Option<&str>) -> AppResult<()> {
    let config = AudioConfig::bundled()?;

    let source = MicrophoneSource::new(config.resampling.clone(), config.capture.clone())?;
    let devices = source.list_devices()?;
    println!("microphones:");
    for device in &devices {
        let mark = if device.is_default { "*" } else { " " };
        println!(
            " {mark} {} ({} Hz, {} channel(s){}{})",
            device.name,
            device.sample_rate_hz,
            device.channels,
            if device.is_built_in { ", built in" } else { "" },
            if device.is_bluetooth {
                ", Bluetooth"
            } else {
                ""
            },
        );
    }

    // Record at the detector's rate so the result can be checked with it.
    let scorer = NeuralSpeechScorer::new();
    let rate = scorer.sample_rate_hz();
    let mut capture = AudioCaptureConfig::new(rate, EXAMPLE_CHUNK_MS);
    if let Some(wanted) = wanted {
        let needle = wanted.to_lowercase();
        let device = devices
            .iter()
            .find(|device| device.name.to_lowercase().contains(&needle))
            .ok_or_else(|| AppError::not_found(format!("no microphone named like '{wanted}'")))?;
        println!("using {}", device.name);
        capture.device_id = Some(device.id.clone());
    } else {
        println!("using the system default");
    }
    let mut session = source.start_capture(capture)?;
    let level = session.level();
    println!("\nrecording for {} s, speak now...", length.as_secs());

    let started = Instant::now();
    let mut samples = Vec::new();
    let mut last_print = Instant::now();
    while started.elapsed() < length && session.is_active() {
        if let Some(chunk) = session.read_chunk(LEVEL_EVERY)? {
            samples.extend_from_slice(&chunk.samples);
        }
        if last_print.elapsed() >= LEVEL_EVERY {
            last_print = Instant::now();
            let value = level.get();
            let bar = "#".repeat((value * 100.0).round() as usize);
            print!(
                "\r{:>5.1} s  level {value:.3} {bar:<60}",
                started.elapsed().as_secs_f32()
            );
            let _ = stdout().flush();
        }
    }
    println!();
    if let Some(reason) = session.interruption() {
        println!("the recording stopped by itself: {reason}");
    }
    let dropouts = session.dropouts();
    samples.extend_from_slice(&session.stop()?.samples);
    let recording = AudioChunk::new(samples, rate, 1);

    let peak = recording
        .samples
        .iter()
        .fold(0.0f32, |top, sample| top.max(sample.abs()));
    println!(
        "recorded {} ms, loudest sample {peak:.3}, glitches reported {dropouts}",
        recording.duration_ms()
    );
    if peak == 0.0 {
        println!(
            "everything is silent: microphone access was probably refused. Allow it in \
             System Settings > Privacy & Security > Microphone for the app that ran this."
        );
    }

    write_wav(output, &recording)?;
    println!("saved {}", output.display());

    let mut detector = VoiceDetector::new(
        config.voice_detection.clone(),
        scorer,
        config.resampling.clone(),
    )?;
    let spans = detector.detect(&recording)?;
    println!("speech found in {} stretch(es):", spans.len());
    for span in spans {
        println!("  {:>6} - {:>6} ms", span.start_ms(), span.end_ms());
    }
    Ok(())
}

/// 16-bit PCM.
fn write_wav(path: &Path, chunk: &AudioChunk) -> AppResult<()> {
    let data_len = (chunk.samples.len() * 2) as u32;
    let mut bytes = Vec::with_capacity(44 + data_len as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&chunk.channels.to_le_bytes());
    bytes.extend_from_slice(&chunk.sample_rate_hz.to_le_bytes());
    bytes.extend_from_slice(&(chunk.sample_rate_hz * 2 * u32::from(chunk.channels)).to_le_bytes());
    bytes.extend_from_slice(&(2 * chunk.channels).to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for sample in &chunk.samples {
        let value = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    fs::write(path, bytes).map_err(|err| AppError::storage(format!("{}: {err}", path.display())))
}
