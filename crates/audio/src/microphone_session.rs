use std::sync::PoisonError;
use std::thread;
use std::time::{Duration, Instant};

use dictation_api::AudioCaptureSession;
use dictation_types::{
    AppError, AppResult, AudioCaptureConfig, AudioChunk, DeviceId, ErrorKind, InputLevel,
};

use crate::capture_device::{Devices, Input, Opened, Recording, Shared, Trouble, Wanted};
use crate::{CaptureBuffer, CaptureTuning, ResampleTuning};

struct Closed;

impl Recording for Closed {
    fn pause(&self) {}
}

/// Conversion runs on the reader's thread, never on the device's audio thread. A stopped
/// stream is reopened (counting a dropout); a lost device falls back to the system default
/// when allowed. A stream silent for the whole patience after opening ends the recording.
pub struct MicrophoneSession {
    devices: Box<dyn Devices>,
    capture: CaptureTuning,
    /// `None` while recording from the system default.
    keeps_to: Option<String>,
    may_fall_back: bool,
    /// `None` once stopped or cancelled.
    input: Option<Input>,
    buffer: CaptureBuffer,
    level: InputLevel,
    /// Glitches of streams replaced earlier, plus one for every replacement.
    earlier_dropouts: u32,
    /// Cleared when audio arrives; a device that keeps stopping is given up once this is
    /// older than the patience.
    lost_since: Option<Instant>,
    last_failure: Option<AppError>,
    next_attempt: Option<Instant>,
    /// Allows reopening the stopped device as the default only once per silence.
    reopened_as_default: bool,
    opened_at: Instant,
    delivered: bool,
    interruption: Option<AppError>,
}

impl MicrophoneSession {
    /// `capture` and `resampling` have already been checked by whoever made them.
    pub fn start(
        devices: Box<dyn Devices>,
        capture: CaptureTuning,
        resampling: &ResampleTuning,
        config: &AudioCaptureConfig,
    ) -> AppResult<Self> {
        // Checked first so a request that cannot be served never turns a microphone on.
        CaptureBuffer::check_output(config.sample_rate_hz, config.frame_duration_ms, resampling)?;
        let level = InputLevel::new();
        let default = Wanted::Default { not: None };
        let (opened, keeps_to) = match config.device_id.as_ref().map(DeviceId::as_str) {
            Some(id) => match devices.open(Wanted::Listed(id), &capture, level.clone()) {
                Err(err) if err.kind == ErrorKind::NotFound && config.fall_back_to_default => {
                    (devices.open(default, &capture, level.clone())?, None)
                }
                opened => (opened?, Some(id.to_owned())),
            },
            None => (devices.open(default, &capture, level.clone())?, None),
        };
        let buffer = CaptureBuffer::new(
            opened.sample_rate_hz,
            opened.channels,
            config.sample_rate_hz,
            config.frame_duration_ms,
            resampling,
        )?;
        Ok(Self {
            devices,
            capture,
            keeps_to,
            may_fall_back: config.device_id.is_none() || config.fall_back_to_default,
            input: Some(opened.input),
            buffer,
            level,
            earlier_dropouts: 0,
            lost_since: None,
            last_failure: None,
            next_attempt: None,
            reopened_as_default: false,
            opened_at: Instant::now(),
            delivered: false,
            interruption: None,
        })
    }

    fn ended() -> AppError {
        AppError::audio("the recording has already ended")
    }

    fn shared(&self) -> Option<&Shared> {
        self.input.as_ref().map(|input| input.shared.as_ref())
    }

    fn can_recover(&self) -> bool {
        self.interruption.is_none()
            && match self.shared().and_then(Shared::trouble) {
                Some(Trouble::Reopen) => true,
                Some(Trouble::Lost) => self.may_fall_back,
                Some(Trouble::Glitch) | None => false,
            }
    }

    fn convert_arrived(&mut self) -> AppResult<()> {
        let Some(shared) = self.shared() else {
            return Ok(());
        };
        let samples = shared.inbox().take_samples();
        if !samples.is_empty() {
            self.lost_since = None;
            self.next_attempt = None;
            self.reopened_as_default = false;
            self.delivered = true;
        }
        self.buffer.push(&samples)
    }

    fn patience(&self) -> Duration {
        Duration::from_millis(self.capture.stream_timeout_ms)
    }

    fn watch_left(&self, now: Instant) -> Option<Duration> {
        (!self.delivered).then(|| (self.opened_at + self.patience()).saturating_duration_since(now))
    }

    fn stall(&self) -> Option<AppError> {
        if self.interruption.is_some() || self.watch_left(Instant::now())? > Duration::ZERO {
            return None;
        }
        let inbox = self.shared()?.inbox();
        (inbox.is_receiving() && !inbox.has_samples()).then(|| {
            AppError::audio(format!(
                "it delivered no audio for {} ms after it was opened",
                self.capture.stream_timeout_ms
            ))
        })
    }

    fn may_reopen_as_default(&self) -> bool {
        self.keeps_to.is_none()
            && !self.reopened_as_default
            && self.shared().and_then(Shared::trouble) == Some(Trouble::Reopen)
    }

    /// Unless `same_default`, the device that stopped is not accepted as the system default.
    fn open_next(&mut self, tuning: &CaptureTuning, same_default: bool) -> AppResult<Opened> {
        let level = self.level.clone();
        let input = self.input.as_ref().ok_or_else(Self::ended)?;
        let lost = input.device_id.clone();
        let trouble = input.shared.trouble();
        if let (Some(Trouble::Reopen), Some(id)) = (trouble, self.keeps_to.clone()) {
            match self
                .devices
                .open(Wanted::Listed(&id), tuning, level.clone())
            {
                Err(err) if err.kind == ErrorKind::NotFound && self.may_fall_back => {}
                opened => return opened,
            }
        }
        let not = if same_default { None } else { lost.as_deref() };
        let opened = self.devices.open(Wanted::Default { not }, tuning, level)?;
        self.keeps_to = None;
        Ok(opened)
    }

    /// The old stream is closed first: a device that allows one stream at a time refuses to
    /// open while the old one exists.
    fn replace_device(&mut self, same_default: bool) -> AppResult<()> {
        let capture = &self.capture;
        let remaining = capture
            .max_recording_ms
            .saturating_sub(self.buffer.converted_ms());
        if remaining == 0 {
            return Err(AppError::timeout(format!(
                "the recording reached its maximum length of {} ms",
                capture.max_recording_ms
            )));
        }
        let shorter = |ms: u32| u32::try_from(remaining).map_or(ms, |left| ms.min(left));
        let tuning = CaptureTuning {
            max_recording_ms: remaining,
            level_window_ms: shorter(capture.level_window_ms),
            stream_timeout_ms: capture.stream_timeout_ms,
            device_switch_poll_ms: capture.device_switch_poll_ms,
            reopen_interval_ms: capture.reopen_interval_ms,
        };

        // Converted while the buffer still has the old stream's format.
        if let Some(old) = &self.input {
            old.stream.pause();
        }
        self.convert_arrived()?;
        if let Some(old) = self.input.as_mut() {
            old.stream = Box::new(Closed);
        }
        let opened = self.open_next(&tuning, same_default)?;
        self.buffer
            .switch_device(opened.sample_rate_hz, opened.channels)?;
        if let Some(old) = self.input.replace(opened.input) {
            self.earlier_dropouts = self
                .earlier_dropouts
                .saturating_add(old.shared.inbox().dropouts())
                .saturating_add(1);
        }
        self.opened_at = Instant::now();
        self.delivered = false;
        Ok(())
    }

    /// A device that could not be found again is reported as a device problem.
    fn ended_because(reason: &AppError) -> AppError {
        let kind = match reason.kind {
            ErrorKind::NotFound => ErrorKind::AudioDevice,
            ref kind => kind.clone(),
        };
        AppError::new(
            kind,
            format!("the microphone stopped and recording could not go on: {reason}"),
        )
    }

    /// Stops the device too: sound arriving after the recording ended is not part of it.
    fn give_up(&mut self, reason: &AppError) {
        self.interruption = Some(Self::ended_because(reason));
        if let Some(input) = &self.input {
            input.stream.pause();
            input.shared.inbox().interrupt(reason.clone());
        }
        self.level.set(0.0);
    }

    /// Returns `true` while still trying when `deadline` passes. A device missing from the
    /// list is waited for, since it may only be away while it is set up again.
    fn recover(&mut self, deadline: Option<Instant>) -> AppResult<bool> {
        if !self.can_recover() {
            return Ok(false);
        }
        self.convert_arrived()?;
        let since = *self.lost_since.get_or_insert_with(Instant::now);
        let poll = Duration::from_millis(self.capture.device_switch_poll_ms);
        let reopen_interval = Duration::from_millis(self.capture.reopen_interval_ms);
        let give_up_at = since + self.patience();
        loop {
            let now = Instant::now();
            if now >= give_up_at {
                if self.may_reopen_as_default() {
                    self.reopened_as_default = true;
                    match self.replace_device(true) {
                        Ok(()) => return Ok(false),
                        Err(err) => self.last_failure = Some(err),
                    }
                }
                let reason = self.last_failure.take().unwrap_or_else(|| {
                    AppError::audio("it kept stopping and delivered no audio in the time allowed")
                });
                self.give_up(&reason);
                return Ok(false);
            }
            if let Some(at) = self.next_attempt.filter(|at| *at > now) {
                let wake = at.min(give_up_at);
                if let Some(deadline) = deadline.filter(|deadline| wake > *deadline) {
                    // The next try is after this read, which still waits its full time.
                    thread::sleep(deadline.saturating_duration_since(now));
                    return Ok(true);
                }
                thread::sleep(wake - now);
                continue;
            }
            match self.replace_device(false) {
                Ok(()) => {
                    self.last_failure = None;
                    self.next_attempt = Some(Instant::now() + reopen_interval);
                    return Ok(false);
                }
                Err(err) if err.kind == ErrorKind::Timeout => {
                    self.give_up(&err);
                    return Ok(false);
                }
                Err(err) => {
                    self.last_failure = Some(err);
                    self.next_attempt = Some(Instant::now() + poll);
                }
            }
        }
    }

    fn close(&mut self) -> AppResult<Input> {
        let input = self.input.take().ok_or_else(Self::ended)?;
        input.stream.pause();
        self.level.set(0.0);
        Ok(input)
    }
}

impl AudioCaptureSession for MicrophoneSession {
    fn read_chunk(&mut self, wait: Duration) -> AppResult<Option<AudioChunk>> {
        if self.input.is_none() {
            return Err(Self::ended());
        }
        let deadline = Instant::now().checked_add(wait);
        loop {
            if let Some(chunk) = self.buffer.take_chunk() {
                return Ok(Some(chunk));
            }
            self.convert_arrived()?;
            if let Some(chunk) = self.buffer.take_chunk() {
                return Ok(Some(chunk));
            }
            if let Some(reason) = self.stall() {
                self.give_up(&reason);
            }
            if self.recover(deadline)? {
                return Ok(None);
            }
            if self.interruption.is_some() {
                return Ok(None);
            }
            // `recover` may have slept, so what is left of the wait is measured after it.
            let now = Instant::now();
            let remaining = match deadline {
                Some(deadline) => deadline.saturating_duration_since(now),
                None => wait,
            };
            if remaining.is_zero() {
                return Ok(None);
            }
            let Some(shared) = self.shared() else {
                return Err(Self::ended());
            };
            let inbox = shared.inbox();
            if !inbox.is_receiving() && !inbox.has_samples() {
                drop(inbox);
                if self.can_recover() {
                    // A reopened stream stopped at once; `recover` paces the next try.
                    continue;
                }
                return Ok(None);
            }
            // Capped so a stream that never delivers is noticed.
            let cap = self
                .watch_left(now)
                .map_or(remaining, |left| remaining.min(left));
            let (inbox, _) = shared
                .arrived
                .wait_timeout_while(inbox, cap, |inbox| {
                    inbox.is_receiving() && !inbox.has_samples()
                })
                .unwrap_or_else(PoisonError::into_inner);
            drop(inbox);
        }
    }

    fn stop(&mut self) -> AppResult<AudioChunk> {
        let input = self.close()?;
        let samples = input.shared.inbox().take_samples();
        self.buffer.push(&samples)?;
        self.buffer.finish()
    }

    fn cancel(&mut self) -> AppResult<()> {
        if let Ok(input) = self.close() {
            input.shared.inbox().take_samples();
            self.buffer.discard();
        }
        Ok(())
    }

    fn is_active(&self) -> bool {
        self.input.is_some()
            && self.interruption.is_none()
            && self.stall().is_none()
            && (self.can_recover()
                || self
                    .shared()
                    .is_some_and(|shared| shared.inbox().is_receiving()))
    }

    fn interruption(&self) -> Option<AppError> {
        if let Some(reason) = &self.interruption {
            return Some(reason.clone());
        }
        if let Some(reason) = self.stall() {
            return Some(Self::ended_because(&reason));
        }
        if self.can_recover() {
            return None;
        }
        self.shared()
            .and_then(|shared| shared.inbox().interruption().cloned())
    }

    fn dropouts(&self) -> u32 {
        let current = self.shared().map_or(0, |shared| shared.inbox().dropouts());
        self.earlier_dropouts.saturating_add(current)
    }

    fn level(&self) -> InputLevel {
        self.level.clone()
    }
}
