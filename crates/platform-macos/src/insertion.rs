use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use dictation_api::TextInjector;
use dictation_types::{
    AppError, AppResult, InjectionWarning, InsertMethod, InsertOutcome, PasteRead, PermissionKind,
};

pub use crate::clipboard_contents::{ClipboardContents, ClipboardSnapshot};
use crate::config::InsertionTuning;
pub use crate::insertion_parts::{
    Clipboard, Keyboard, MainThread, OfferNews, SecureInput, TextOffer,
};
pub use crate::insertion_wait::{WaitStep, wait_step};

/// The most UTF-16 code units the system takes in one typed event; it drops the rest.
pub const MAX_TYPED_UNITS: usize = 20;

/// Pastes run one at a time, in the order they were asked for, so a second paste never saves
/// the first one's text as the clipboard to put back and dictated text arrives in order. A
/// paste whose text has been read stops waiting as soon as another paste is asked for.
pub struct Inserter<C, K, S, M> {
    clipboard: C,
    keyboard: K,
    secure_input: S,
    main: M,
    tuning: Mutex<InsertionTuning>,
    turns: Mutex<Turns>,
    turn_over: Condvar,
}

/// Pastes take numbered turns; `serving` is the turn now running.
#[derive(Default)]
struct Turns {
    next: u64,
    serving: u64,
    /// The text of the running paste while it waits for readers.
    waiting: Option<Arc<TextOffer>>,
}

impl Turns {
    fn others_queued(&self) -> bool {
        self.next > self.serving + 1
    }
}

/// Hands the turn to the next paste when dropped, however the paste ended.
struct Turn<'a> {
    turns: &'a Mutex<Turns>,
    turn_over: &'a Condvar,
}

impl Drop for Turn<'_> {
    fn drop(&mut self) {
        let mut turns = self.turns.lock().unwrap_or_else(PoisonError::into_inner);
        turns.serving += 1;
        turns.waiting = None;
        self.turn_over.notify_all();
    }
}

/// The text promised on the clipboard: the change count it went on at, and what it hears.
struct Ours<'a> {
    count: i64,
    offer: &'a TextOffer,
}

const LEFT_ALONE: &str =
    "something else changed the clipboard while pasting, so it was left as it is";
const PARTLY_PUT_BACK: &str =
    "part of the clipboard could not be read, so only the rest was put back";
const NOT_PUT_BACK: &str =
    "the clipboard held something that could not be read, so it was not put back";

fn paste_read(news: &OfferNews) -> PasteRead {
    if news.last_read.is_some() {
        PasteRead::Read
    } else if news.taken_before_keys {
        PasteRead::TakenBeforeKeys
    } else {
        PasteRead::Unread
    }
}

fn with_restore_failure(err: AppError, restore: &AppError) -> AppError {
    AppError {
        message: format!(
            "{}; putting the clipboard back failed too: {}",
            err.message, restore.message
        ),
        ..err
    }
}

/// Splits `text` into runs of at most [`MAX_TYPED_UNITS`] UTF-16 code units, never splitting a
/// character in two.
fn typed_runs(text: &str) -> Vec<Vec<u16>> {
    let mut runs = Vec::new();
    let mut run: Vec<u16> = Vec::new();
    let mut buffer = [0_u16; 2];
    for ch in text.chars() {
        let units = ch.encode_utf16(&mut buffer);
        if run.len() + units.len() > MAX_TYPED_UNITS {
            runs.push(std::mem::take(&mut run));
        }
        run.extend_from_slice(units);
    }
    if !run.is_empty() {
        runs.push(run);
    }
    runs
}

impl<C: Clipboard, K: Keyboard, S: SecureInput, M: MainThread> Inserter<C, K, S, M> {
    pub fn with_parts(
        clipboard: C,
        keyboard: K,
        secure_input: S,
        main: M,
        tuning: InsertionTuning,
    ) -> Self {
        Self {
            clipboard,
            keyboard,
            secure_input,
            main,
            tuning: Mutex::new(tuning),
            turns: Mutex::new(Turns::default()),
            turn_over: Condvar::new(),
        }
    }

    fn turns(&self) -> MutexGuard<'_, Turns> {
        self.turns.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Waits for the pastes asked for before this one, telling the running one that this one is
    /// waiting. Refused on the main thread: the running paste needs it to answer the application
    /// and to put the clipboard back, so waiting there would never end.
    fn take_turn(&self) -> AppResult<Turn<'_>> {
        if self.main.is_current() {
            return Err(AppError::internal(
                "pasting waits for the main thread, so it cannot be asked for on the main thread",
            ));
        }
        let mut turns = self.turns();
        let mine = turns.next;
        turns.next += 1;
        if let Some(offer) = &turns.waiting {
            offer.note_superseded();
        }
        while turns.serving != mine {
            turns = self
                .turn_over
                .wait(turns)
                .unwrap_or_else(PoisonError::into_inner);
        }
        Ok(Turn {
            turns: &self.turns,
            turn_over: &self.turn_over,
        })
    }

    /// Takes effect from the next paste on; a paste already waiting keeps its waits.
    pub fn set_tuning(&self, tuning: InsertionTuning) {
        *self.tuning.lock().unwrap_or_else(PoisonError::into_inner) = tuning;
    }

    /// Fails before anything is touched when the system would drop the keys sent.
    fn check_keys_allowed(&self) -> AppResult<()> {
        if self.keyboard.can_send() {
            Ok(())
        } else {
            Err(AppError::permission_denied(
                Some(PermissionKind::Accessibility),
                "macOS drops keys this application sends until it has the Accessibility permission",
            ))
        }
    }

    /// The text is promised rather than written, so the clipboard goes back once the application
    /// has read it instead of after a guess at how long that takes.
    fn paste(&self, text: &str) -> AppResult<InsertOutcome> {
        let _turn = self.take_turn()?;
        self.check_keys_allowed()?;
        let offer = Arc::new(TextOffer::new());
        // Read and promised in one go, leaving as little room as there can be for a copy made in
        // between, which the promise would replace unseen. Our own text may still be promised
        // from a paste whose clipboard was not put back, and the system wants promises kept on
        // the main thread.
        let (earlier, offered) = self.main.run(|| {
            let earlier = self.clipboard.contents()?;
            AppResult::Ok((earlier, self.clipboard.offer_text(text, &offer)))
        })?;
        let count = match offered {
            Ok(count) => count,
            // The clipboard may have been emptied before the text was refused.
            Err(err) => return Err(self.undo(err, earlier.readable(), None)),
        };
        let ours = Ours {
            count,
            offer: &offer,
        };
        let sent = self.main.run(|| {
            offer.start_counting();
            self.keyboard.paste()
        });
        let partial = matches!(earlier, ClipboardSnapshot::Partial(_));
        let Some(earlier) = earlier.readable() else {
            if let Err(err) = sent {
                return Err(AppError {
                    message: format!("{}; {NOT_PUT_BACK}", err.message),
                    ..err
                });
            }
            let mut outcome = self.outcome(InsertMethod::PasteWithClipboardRestore);
            outcome.clipboard_restored = false;
            outcome.diagnostic = Some(NOT_PUT_BACK.to_owned());
            return Ok(outcome);
        };
        self.wait_for_readers(&offer, sent.is_ok());
        if let Err(err) = sent {
            return Err(self.undo(err, Some(earlier), Some(ours)));
        }
        let mut outcome = self.outcome(InsertMethod::PasteWithClipboardRestore);
        outcome.paste_read = Some(paste_read(&offer.news()));
        match self.put_back(earlier, Some(ours)) {
            Ok(true) => {
                if partial {
                    outcome.diagnostic = Some(PARTLY_PUT_BACK.to_owned());
                }
            }
            Ok(false) => {
                outcome.clipboard_restored = false;
                outcome.diagnostic = Some(LEFT_ALONE.to_owned());
            }
            Err(err) => {
                outcome.clipboard_restored = false;
                outcome.diagnostic = Some(format!(
                    "pasted, but putting the clipboard back failed: {}",
                    err.message
                ));
            }
        }
        Ok(outcome)
    }

    /// Waits off the main thread, which answers the reads, until [`wait_step`] says to stop.
    fn wait_for_readers(&self, offer: &Arc<TextOffer>, keys_sent: bool) {
        {
            let mut turns = self.turns();
            if turns.others_queued() {
                offer.note_superseded();
            }
            turns.waiting = Some(Arc::clone(offer));
        }
        let tuning = self
            .tuning
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let started = Instant::now();
        loop {
            let news = offer.news();
            match wait_step(&tuning, &news, started, Instant::now(), keys_sent) {
                WaitStep::Done => return,
                WaitStep::Wait(left) => offer.wait_for_news(&news, left),
            }
        }
    }

    /// Puts the clipboard back after a step failed, and returns the error to report.
    fn undo(
        &self,
        err: AppError,
        earlier: Option<&ClipboardContents>,
        ours: Option<Ours<'_>>,
    ) -> AppError {
        let Some(earlier) = earlier else {
            return err;
        };
        match self.put_back(earlier, ours) {
            Ok(_) => err,
            Err(restore) => with_restore_failure(err, &restore),
        }
    }

    /// Puts `earlier` back on the main thread unless something else has taken the clipboard
    /// over since our text went on it. Without `ours`, our text never went on, and it is put
    /// back regardless. Returns whether it was put back.
    fn put_back(&self, earlier: &ClipboardContents, ours: Option<Ours<'_>>) -> AppResult<bool> {
        self.main.run(|| {
            if let Some(ours) = ours
                && (ours.offer.news().replaced || self.clipboard.change_count()? != ours.count)
            {
                return Ok(false);
            }
            self.clipboard.restore(earlier)?;
            Ok(true)
        })
    }

    /// Written for keeps rather than promised, so clipboard history tools may keep it too.
    fn paste_keeping_text(&self, text: &str) -> AppResult<InsertOutcome> {
        let _turn = self.take_turn()?;
        self.check_keys_allowed()?;
        self.clipboard.write_text(text, false)?;
        self.main.run(|| self.keyboard.paste())?;
        Ok(self.outcome(InsertMethod::PasteKeepingText))
    }

    fn type_text(&self, text: &str) -> AppResult<InsertOutcome> {
        self.check_keys_allowed()?;
        for run in typed_runs(text) {
            self.keyboard.type_units(&run)?;
        }
        Ok(self.outcome(InsertMethod::SimulatedTyping))
    }

    /// A successful outcome, with the secure input warning when it is on.
    fn outcome(&self, method: InsertMethod) -> InsertOutcome {
        let mut outcome = InsertOutcome::success(method);
        outcome.warning = self.check_injection_warning();
        outcome
    }

    fn copy(&self, text: &str) -> AppResult<InsertOutcome> {
        self.clipboard.write_text(text, false)?;
        Ok(InsertOutcome::success(InsertMethod::CopyOnly))
    }
}

impl<C: Clipboard, K: Keyboard, S: SecureInput, M: MainThread> TextInjector
    for Inserter<C, K, S, M>
{
    fn insert(&self, text: &str, method: InsertMethod) -> AppResult<InsertOutcome> {
        match method {
            InsertMethod::PasteWithClipboardRestore => self.paste(text),
            InsertMethod::PasteKeepingText => self.paste_keeping_text(text),
            InsertMethod::SimulatedTyping => self.type_text(text),
            InsertMethod::CopyOnly => self.copy(text),
        }
    }

    fn check_injection_warning(&self) -> Option<InjectionWarning> {
        self.secure_input
            .is_active()
            .then_some(InjectionWarning::SecureInputActive)
    }
}
