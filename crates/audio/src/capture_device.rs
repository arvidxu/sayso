//! The device side of a recording, kept free of real devices so tests can use stand-ins.

use std::fmt::Display;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

use cpal::ErrorKind as DeviceErrorKind;
use dictation_types::{AppError, AppResult, InputLevel};

use crate::{CaptureTuning, DeviceInbox};

/// What a problem a device reports means for the recording.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trouble {
    /// The stream goes on; the problem is counted as a dropout.
    Glitch,
    /// The stream stopped, but the device may still be there: it is opened again, and the
    /// recording goes on if it opens.
    Reopen,
    Lost,
}

impl Trouble {
    pub fn of(kind: DeviceErrorKind) -> Self {
        match kind {
            DeviceErrorKind::Xrun
            | DeviceErrorKind::DeviceChanged
            | DeviceErrorKind::RealtimeDenied => Self::Glitch,
            // Some systems invalidate the stream when the rate changes for a call, others stop
            // capturing after any problem. Only reopening shows whether the device is gone.
            DeviceErrorKind::StreamInvalidated
            | DeviceErrorKind::DeviceNotAvailable
            | DeviceErrorKind::DeviceBusy => Self::Reopen,
            _ => Self::Lost,
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What a device's audio thread and the reader share.
pub struct Shared {
    inbox: Mutex<DeviceInbox>,
    pub(crate) arrived: Condvar,
    /// What the problem that stopped the stream means, when a problem of the device did.
    trouble: Mutex<Option<Trouble>>,
}

impl Shared {
    pub fn new(
        sample_rate_hz: u32,
        channels: u16,
        tuning: &CaptureTuning,
        level: InputLevel,
    ) -> AppResult<Arc<Self>> {
        Ok(Arc::new(Self {
            inbox: Mutex::new(DeviceInbox::new(sample_rate_hz, channels, tuning, level)?),
            arrived: Condvar::new(),
            trouble: Mutex::new(None),
        }))
    }

    pub(crate) fn inbox(&self) -> MutexGuard<'_, DeviceInbox> {
        lock(&self.inbox)
    }

    /// Takes in interleaved samples the device delivered, already as numbers between -1 and
    /// 1, and wakes the reader.
    pub fn push(&self, samples: impl IntoIterator<Item = f32>) {
        self.inbox().push(samples);
        self.arrived.notify_all();
    }

    /// A glitch is counted and the stream goes on; anything else stops the stream, keeping the
    /// trouble unless the stream had already stopped for another reason.
    pub fn report(&self, trouble: Trouble, problem: impl Display) {
        let mut inbox = self.inbox();
        if trouble == Trouble::Glitch {
            inbox.note_dropout();
        } else {
            if inbox.is_receiving() {
                *lock(&self.trouble) = Some(trouble);
            }
            inbox.interrupt(AppError::audio(format!(
                "the microphone stopped: {problem}"
            )));
        }
        drop(inbox);
        self.arrived.notify_all();
    }

    pub fn trouble(&self) -> Option<Trouble> {
        *lock(&self.trouble)
    }
}

/// Dropping it stops the device.
pub trait Recording: Send {
    /// Stops audio from arriving, keeping the stream.
    fn pause(&self);
}

pub struct Input {
    pub stream: Box<dyn Recording>,
    pub shared: Arc<Shared>,
    pub device_id: Option<String>,
}

pub struct Opened {
    pub input: Input,
    pub sample_rate_hz: u32,
    pub channels: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wanted<'a> {
    /// Not finding it fails as not found.
    Listed(&'a str),
    /// The system default, unless it is the device listed under `not`.
    Default { not: Option<&'a str> },
}

pub trait Devices: Send {
    fn open(
        &self,
        wanted: Wanted<'_>,
        tuning: &CaptureTuning,
        level: InputLevel,
    ) -> AppResult<Opened>;
}
