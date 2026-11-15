use std::mem;

use dictation_types::{AppError, AppResult, AudioChunk};

use crate::downmix_to_mono;
use crate::measure::{frames_to_ms, ms_to_frames};
use crate::speech_span::{SpeechSpan, check_spans};

/// The limits depend on the speech engine, so everything comes from the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentationConfig {
    /// A piece that has reached this length is closed at the next pause.
    pub target_segment_ms: u64,
    /// Speech without a pause is cut to fit.
    pub max_segment_ms: u64,
    /// Silence put where a pause was removed between joined spans; never inside unbroken speech.
    pub gap_ms: u32,
    /// Applies when unbroken speech is cut; the last part may be shorter.
    pub min_part_ms: u64,
    /// The cut is placed in the middle of the quietest window of this length.
    pub quiet_window_ms: u32,
}

impl SegmentationConfig {
    pub fn validate(&self) -> AppResult<()> {
        if self.target_segment_ms == 0 {
            return Err(AppError::validation(
                "target_segment_ms must be greater than zero",
            ));
        }
        if self.max_segment_ms < self.target_segment_ms {
            return Err(AppError::validation(
                "max_segment_ms must not be less than target_segment_ms",
            ));
        }
        if self.min_part_ms == 0 || self.min_part_ms > self.max_segment_ms {
            return Err(AppError::validation(
                "min_part_ms must be greater than zero and not above max_segment_ms",
            ));
        }
        if self.quiet_window_ms == 0 {
            return Err(AppError::validation(
                "quiet_window_ms must be greater than zero",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioSegment {
    pub index: usize,
    /// Mono at the recording's sample rate.
    pub audio: AudioChunk,
    pub start_frame: usize,
    /// Exclusive.
    pub end_frame: usize,
    pub spans: Vec<SpeechSpan>,
}

impl AudioSegment {
    pub fn start_ms(&self) -> u64 {
        frames_to_ms(self.start_frame, self.audio.sample_rate_hz)
    }

    pub fn end_ms(&self) -> u64 {
        frames_to_ms(self.end_frame, self.audio.sample_rate_hz)
    }

    /// Includes inserted gaps.
    pub fn duration_ms(&self) -> u64 {
        self.audio.duration_ms()
    }
}

/// Every span frame lands in exactly one piece, in order. An overlong span is cut at the
/// quietest window; ties go to the latest so evenly loud speech gets as few parts as possible.
pub fn segment_speech(
    chunk: &AudioChunk,
    spans: &[SpeechSpan],
    config: &SegmentationConfig,
) -> AppResult<Vec<AudioSegment>> {
    config.validate()?;
    let mono = downmix_to_mono(&chunk.samples, chunk.channels)?;
    check_spans(chunk, mono.len(), spans)?;
    let rate = chunk.sample_rate_hz;

    let max_frames = ms_to_frames(config.max_segment_ms, rate).max(1);
    let target_frames = ms_to_frames(config.target_segment_ms, rate).clamp(1, max_frames);
    let gap_frames = ms_to_frames(u64::from(config.gap_ms), rate);
    let min_part_frames = ms_to_frames(config.min_part_ms, rate).clamp(1, max_frames);
    let window_frames = ms_to_frames(u64::from(config.quiet_window_ms), rate).max(1);

    let mut parts: Vec<Part> = Vec::with_capacity(spans.len());
    for span in spans.iter().filter(|span| span.frames() > 0) {
        split_long_span(
            &mono,
            span,
            min_part_frames,
            max_frames,
            window_frames,
            &mut parts,
        );
    }

    let mut previous_end: Option<usize> = None;
    for part in &mut parts {
        part.continues_previous = previous_end == Some(part.span.start_frame);
        previous_end = Some(part.span.end_frame);
    }

    let mut segments = Vec::new();
    let mut group: Vec<Part> = Vec::new();
    let mut group_frames = 0usize;
    for part in parts {
        let gap = if part.continues_previous {
            0
        } else {
            gap_frames
        };
        let projected = group_frames + gap + part.span.frames();
        if !group.is_empty() && (group_frames >= target_frames || projected > max_frames) {
            let index = segments.len();
            segments.push(build_segment(
                index,
                &mono,
                rate,
                mem::take(&mut group),
                gap_frames,
            ));
        }
        group_frames = if group.is_empty() {
            part.span.frames()
        } else {
            projected
        };
        group.push(part);
    }
    if !group.is_empty() {
        let index = segments.len();
        segments.push(build_segment(index, &mono, rate, group, gap_frames));
    }
    Ok(segments)
}

struct Part {
    span: SpeechSpan,
    continues_previous: bool,
}

fn build_segment(
    index: usize,
    mono: &[f32],
    sample_rate_hz: u32,
    parts: Vec<Part>,
    gap_frames: usize,
) -> AudioSegment {
    let start_frame = parts.first().map_or(0, |part| part.span.start_frame);
    let end_frame = parts.last().map_or(0, |part| part.span.end_frame);
    let mut audio = Vec::new();
    for (position, part) in parts.iter().enumerate() {
        if position > 0 && !part.continues_previous {
            audio.resize(audio.len() + gap_frames, 0.0);
        }
        audio.extend_from_slice(&mono[part.span.start_frame..part.span.end_frame]);
    }
    AudioSegment {
        index,
        audio: AudioChunk::new(audio, sample_rate_hz, 1),
        start_frame,
        end_frame,
        spans: parts.into_iter().map(|part| part.span).collect(),
    }
}

fn split_long_span(
    mono: &[f32],
    span: &SpeechSpan,
    min_part_frames: usize,
    max_frames: usize,
    window_frames: usize,
    out: &mut Vec<Part>,
) {
    let part = |start: usize, end: usize| Part {
        span: SpeechSpan {
            start_frame: start,
            end_frame: end,
            ..span.clone()
        },
        continues_previous: false,
    };
    let mut cursor = span.start_frame;
    while span.end_frame - cursor > max_frames {
        // Leaves the minimum part on both sides so the speech after a cut is never a sliver.
        let earliest = cursor + min_part_frames;
        let latest = (cursor + max_frames).min(span.end_frame - min_part_frames);
        let cut = if earliest <= latest {
            quietest_point(mono, window_frames, earliest, latest).clamp(earliest, latest)
        } else {
            // No room for two minimum parts: only the last part is short.
            cursor + max_frames
        };
        out.push(part(cursor, cut));
        cursor = cut;
    }
    if span.end_frame > cursor {
        out.push(part(cursor, span.end_frame));
    }
}

/// The middle of the quietest window, the latest on a tie; `to` when the range cannot hold one.
fn quietest_point(mono: &[f32], window: usize, from: usize, to: usize) -> usize {
    if to < from + window {
        return to;
    }
    let step = window.div_ceil(2);
    let mut best = (f32::INFINITY, to);
    let mut position = to - window;
    loop {
        let energy: f32 = mono[position..position + window]
            .iter()
            .map(|sample| sample * sample)
            .sum();
        if energy < best.0 {
            best = (energy, position + window / 2);
        }
        if position < from + step {
            break;
        }
        position -= step;
    }
    best.1
}
