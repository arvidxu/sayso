//! Handing shortcut notices to the listener on a thread of its own, one at a time and in
//! order, so neither the keyboard watcher nor a caller of the source ever runs the listener.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::{self, JoinHandle};

use dictation_api::ShortcutListener;
use dictation_types::{AppError, AppResult, HotkeyNotice};

pub(crate) enum Message {
    Notice(HotkeyNotice),
    Replace(ShortcutListener),
}

/// It ends once every sender is gone and what was queued
/// before has been delivered.
pub(crate) struct Delivery {
    thread: JoinHandle<()>,
}

impl Delivery {
    pub(crate) fn start(listener: ShortcutListener) -> AppResult<(Self, Sender<Message>)> {
        let (sender, receiver) = channel();
        let thread = thread::Builder::new()
            .name("shortcut-delivery".to_owned())
            .spawn(move || deliver(listener, &receiver))
            .map_err(|err| {
                AppError::hotkey(format!("could not start delivering shortcuts: {err}"))
            })?;
        Ok((Self { thread }, sender))
    }

    /// Waits until the thread has delivered what was queued and ended. Its sender must be gone
    /// already. On the delivery thread itself, from inside the listener, it cannot wait: the
    /// thread ends after the current call returns and the rest of the queue is delivered.
    pub(crate) fn finish(self) {
        if self.thread.thread().id() != thread::current().id() {
            let _ = self.thread.join();
        }
    }
}

fn deliver(mut listener: ShortcutListener, receiver: &Receiver<Message>) {
    while let Ok(message) = receiver.recv() {
        match message {
            Message::Replace(next) => listener = next,
            Message::Notice(notice) => {
                // A listener that panics loses that notice, not the ones after it.
                let _ = catch_unwind(AssertUnwindSafe(|| listener(notice)));
            }
        }
    }
}
