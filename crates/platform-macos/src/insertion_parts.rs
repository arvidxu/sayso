//! The system parts insertion uses, as small traits so the order of the steps can be tested.

use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use dictation_types::AppResult;

use crate::clipboard_contents::{ClipboardContents, ClipboardSnapshot};

pub trait Clipboard: Send + Sync {
    /// Everything on the clipboard now, in every type it is offered in, or that it holds
    /// something that could not be read.
    fn contents(&self) -> AppResult<ClipboardSnapshot>;

    /// `transient` marks it as about to be replaced, so
    /// clipboard history tools leave it out. Returns the clipboard's change count afterwards.
    /// When it fails, the clipboard may already have been emptied.
    fn write_text(&self, text: &str, transient: bool) -> AppResult<i64>;

    /// Promises `text` on the clipboard, marked transient, without handing it over yet: each
    /// time an application asks for it, the text is handed over and `offer` hears of the read;
    /// once something else takes the clipboard over, `offer` hears of that. The promise is kept
    /// until the clipboard is written again. Returns the clipboard's change count afterwards.
    /// Called on the main thread, where the system asks for promised data.
    fn offer_text(&self, text: &str, offer: &Arc<TextOffer>) -> AppResult<i64>;

    /// Empty contents leave it empty.
    fn restore(&self, contents: &ClipboardContents) -> AppResult<()>;

    /// A number that goes up each time anything changes the clipboard.
    fn change_count(&self) -> AppResult<i64>;
}

pub trait Keyboard: Send + Sync {
    /// Without the Accessibility permission
    /// sent keys are dropped silently, so this is checked first.
    fn can_send(&self) -> bool;

    /// Called on the main thread.
    fn paste(&self) -> AppResult<()>;

    /// Types `text`, given as UTF-16 code units. The caller keeps each call within
    /// [`crate::insertion::MAX_TYPED_UNITS`] and never splits a character between calls.
    fn type_units(&self, text: &[u16]) -> AppResult<()>;
}

/// Whether the system is keeping keystrokes from other programs, as it does while a password
/// field has focus. While it is, global shortcuts are not seen. Whether pasted or typed text
/// still arrives is not known for certain, so insertion goes ahead and reports a warning.
pub trait SecureInput: Send + Sync {
    fn is_active(&self) -> bool;
}

/// The thread whose run loop answers applications asking for promised clipboard data. The
/// promise and the paste keys go out on it, and the wait for readers must leave it free.
pub trait MainThread: Send + Sync {
    /// Runs `work` there and returns what it returns.
    fn run<R: Send>(&self, work: impl FnOnce() -> R + Send) -> R;

    fn is_current(&self) -> bool;
}

/// What has happened to promised text since the paste keys went out.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OfferNews {
    /// When an application last asked for the text.
    pub last_read: Option<Instant>,
    /// Something else has taken the clipboard over.
    pub replaced: bool,
    /// Another paste is waiting for this one to finish.
    pub superseded: bool,
    /// Something asked for the text before the paste keys went out, so the system answers
    /// later readers from its own copy and they are not heard.
    pub taken_before_keys: bool,
    /// Goes up with every piece of news, to tell news from none.
    pub changes: u64,
}

#[derive(Debug, Default)]
struct OfferState {
    counting: bool,
    news: OfferNews,
}

/// Hears what happens to text promised on the clipboard. Reads count only from
/// [`Self::start_counting`] on, so a clipboard history tool reading the text before the paste
/// keys go out does not pass for the application pasting.
#[derive(Debug, Default)]
pub struct TextOffer {
    state: Mutex<OfferState>,
    changed: Condvar,
}

impl TextOffer {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, OfferState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn start_counting(&self) {
        self.lock().counting = true;
    }

    pub fn note_read(&self) {
        let mut state = self.lock();
        if state.counting {
            state.news.last_read = Some(Instant::now());
            state.news.changes += 1;
            self.changed.notify_all();
        } else {
            state.news.taken_before_keys = true;
        }
    }

    pub fn note_replaced(&self) {
        let mut state = self.lock();
        state.news.replaced = true;
        state.news.changes += 1;
        self.changed.notify_all();
    }

    pub fn note_superseded(&self) {
        let mut state = self.lock();
        state.news.superseded = true;
        state.news.changes += 1;
        self.changed.notify_all();
    }

    pub fn news(&self) -> OfferNews {
        self.lock().news
    }

    /// Returns at once when there has been news since `seen`.
    pub fn wait_for_news(&self, seen: &OfferNews, timeout: Duration) {
        let state = self.lock();
        let _ = self
            .changed
            .wait_timeout_while(state, timeout, |state| state.news.changes == seen.changes)
            .unwrap_or_else(PoisonError::into_inner);
    }
}
