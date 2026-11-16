#![allow(dead_code)]

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use dictation_types::{AppError, AppResult};
use platform_macos::insertion::MAX_TYPED_UNITS;
use platform_macos::{
    Clipboard, ClipboardContents, ClipboardSnapshot, Inserter, InsertionTuning, Keyboard,
    MainThread, SecureInput, TextOffer,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    Read,
    Write {
        text: String,
        transient: bool,
    },
    /// The text promised, always marked transient.
    Offer(String),
    Restore(ClipboardContents),
    Paste,
    Type(String),
}

#[derive(Default)]
pub struct World {
    /// When sending the paste keys returned or failed.
    pub pasted_at: Option<Instant>,
    pub restored_at: Option<Instant>,
    pub steps: Vec<Step>,
    pub contents: ClipboardContents,
    pub count: i64,
    pub fail_paste: bool,
    pub fail_restore: bool,
    /// Writing the text empties the clipboard and then fails.
    pub fail_write: bool,
    /// The system keeps the contents from being read.
    pub unreadable: bool,
    /// Part of the contents cannot be read; the rest, `contents`, can.
    pub partly_unreadable: bool,
    /// The system drops the keys sent, as without the Accessibility permission.
    pub keys_dropped: bool,
    /// How long sending the paste takes, to let two pastes overlap.
    pub paste_takes: Duration,
    pub fail_typing_after: Option<usize>,
    /// Something else writes to the clipboard right after the paste is sent.
    pub other_writer: bool,
    pub secure: bool,
    /// No application asks for the promised text.
    pub nobody_reads: bool,
    /// Something asks for the promised text as soon as it is promised, before the paste keys
    /// go out; from then on the system answers readers itself.
    pub read_before_keys: bool,
    /// Further reads, this many milliseconds after the paste, besides the one at once.
    pub later_reads_ms: Vec<u64>,
    /// Something else writes to the clipboard this many milliseconds after the paste.
    pub replaced_after_ms: Option<u64>,
    /// Something else takes the clipboard over without its change count moving, as only the
    /// promise hears.
    pub owner_changed_after_ms: Option<u64>,
    /// How many main thread calls are running now.
    pub on_main: usize,
    /// Steps that should have run on the main thread and did not.
    pub off_main: Vec<Step>,
    /// The latest promise.
    pub offer: Option<Arc<TextOffer>>,
    /// The caller stands in for the main thread.
    pub caller_is_main: bool,
}

impl World {
    fn note_thread(&mut self, step: &Step) {
        if self.on_main == 0 {
            self.off_main.push(step.clone());
        }
    }
}

#[derive(Clone, Default)]
pub struct Fake(pub Arc<Mutex<World>>);

impl Fake {
    pub fn with(change: impl FnOnce(&mut World)) -> Self {
        let fake = Self::default();
        change(&mut fake.0.lock().unwrap());
        fake
    }

    pub fn steps(&self) -> Vec<Step> {
        self.0.lock().unwrap().steps.clone()
    }

    pub fn contents(&self) -> ClipboardContents {
        self.0.lock().unwrap().contents.clone()
    }

    /// Puts the clipboard back as soon as the text was read; a test that leaves the text unread
    /// sets its own waits.
    pub fn inserter(&self) -> Inserter<Fake, Fake, Fake, Fake> {
        self.inserter_with(tuning(0, 2_000, 0))
    }

    pub fn inserter_with(&self, tuning: InsertionTuning) -> Inserter<Fake, Fake, Fake, Fake> {
        Inserter::with_parts(
            self.clone(),
            self.clone(),
            self.clone(),
            self.clone(),
            tuning,
        )
    }

    /// Runs `change` on the world `after_ms` from now, on a thread of its own.
    fn later(&self, after_ms: u64, change: impl FnOnce(&mut World) + Send + 'static) {
        let fake = self.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(after_ms));
            change(&mut fake.0.lock().unwrap());
        });
    }
}

pub fn tuning(quiet_ms: u64, unread_ms: u64, failed_ms: u64) -> InsertionTuning {
    InsertionTuning {
        restore_quiet_after_read_ms: quiet_ms,
        restore_unread_timeout_ms: unread_ms,
        restore_after_failed_paste_ms: failed_ms,
    }
}

/// Another program's write: new contents and a new change count, which the promise hears of.
fn written_by_someone_else(world: &mut World) {
    world.contents = text_contents("copied by the user");
    world.count += 1;
    if let Some(offer) = &world.offer {
        offer.note_replaced();
    }
}

pub fn text_contents(text: &str) -> ClipboardContents {
    ClipboardContents {
        items: vec![vec![(
            "public.utf8-plain-text".to_owned(),
            text.as_bytes().to_vec(),
        )]],
    }
}

pub fn image_and_text() -> ClipboardContents {
    ClipboardContents {
        items: vec![
            vec![("public.png".to_owned(), vec![0x89, 0x50, 0x4E, 0x47])],
            vec![
                ("public.utf8-plain-text".to_owned(), b"caption".to_vec()),
                ("public.html".to_owned(), b"<b>caption</b>".to_vec()),
            ],
        ],
    }
}

impl Clipboard for Fake {
    fn contents(&self) -> AppResult<ClipboardSnapshot> {
        let mut world = self.0.lock().unwrap();
        world.note_thread(&Step::Read);
        world.steps.push(Step::Read);
        if world.unreadable {
            return Ok(ClipboardSnapshot::Unreadable);
        }
        if world.partly_unreadable {
            return Ok(ClipboardSnapshot::Partial(world.contents.clone()));
        }
        Ok(ClipboardSnapshot::Contents(world.contents.clone()))
    }

    fn write_text(&self, text: &str, transient: bool) -> AppResult<i64> {
        let mut world = self.0.lock().unwrap();
        if world.fail_write {
            world.contents = ClipboardContents::default();
            world.count += 1;
            return Err(AppError::injection("the clipboard did not take the text"));
        }
        world.steps.push(Step::Write {
            text: text.to_owned(),
            transient,
        });
        world.contents = text_contents(text);
        world.count += 1;
        Ok(world.count)
    }

    fn offer_text(&self, text: &str, offer: &Arc<TextOffer>) -> AppResult<i64> {
        let mut world = self.0.lock().unwrap();
        if world.fail_write {
            world.contents = ClipboardContents::default();
            world.count += 1;
            return Err(AppError::injection("the clipboard did not take the text"));
        }
        let step = Step::Offer(text.to_owned());
        world.note_thread(&step);
        world.steps.push(step);
        world.contents = text_contents(text);
        world.count += 1;
        world.offer = Some(Arc::clone(offer));
        if world.read_before_keys {
            offer.note_read();
        }
        Ok(world.count)
    }

    fn restore(&self, contents: &ClipboardContents) -> AppResult<()> {
        let mut world = self.0.lock().unwrap();
        world.note_thread(&Step::Restore(contents.clone()));
        if world.fail_restore {
            return Err(AppError::injection("the clipboard is locked"));
        }
        world.steps.push(Step::Restore(contents.clone()));
        world.restored_at = Some(Instant::now());
        world.contents = contents.clone();
        world.count += 1;
        Ok(())
    }

    fn change_count(&self) -> AppResult<i64> {
        Ok(self.0.lock().unwrap().count)
    }
}

impl Keyboard for Fake {
    fn can_send(&self) -> bool {
        !self.0.lock().unwrap().keys_dropped
    }

    fn paste(&self) -> AppResult<()> {
        let takes = self.0.lock().unwrap().paste_takes;
        std::thread::sleep(takes);
        let mut world = self.0.lock().unwrap();
        world.note_thread(&Step::Paste);
        world.pasted_at = Some(Instant::now());
        if world.fail_paste {
            return Err(AppError::injection("no keyboard events allowed"));
        }
        world.steps.push(Step::Paste);
        if world.other_writer {
            written_by_someone_else(&mut world);
        }
        // Text written for keeps has no promise to hear of reads.
        let Some(offer) = world.offer.clone() else {
            return Ok(());
        };
        if !world.nobody_reads && !world.read_before_keys {
            offer.note_read();
        }
        for after in world.later_reads_ms.clone() {
            let offer = Arc::clone(&offer);
            self.later(after, move |_| offer.note_read());
        }
        if let Some(after) = world.replaced_after_ms {
            self.later(after, written_by_someone_else);
        }
        if let Some(after) = world.owner_changed_after_ms {
            self.later(after, move |_| offer.note_replaced());
        }
        Ok(())
    }

    fn type_units(&self, text: &[u16]) -> AppResult<()> {
        let mut world = self.0.lock().unwrap();
        let typed = world
            .steps
            .iter()
            .filter(|s| matches!(s, Step::Type(_)))
            .count();
        if world.fail_typing_after == Some(typed) {
            return Err(AppError::injection("typing stopped"));
        }
        assert!(text.len() <= MAX_TYPED_UNITS);
        world.steps.push(Step::Type(
            String::from_utf16(text).expect("whole characters"),
        ));
        Ok(())
    }
}

/// The calling thread stands in for the main thread; the steps that belong there are marked.
impl MainThread for Fake {
    fn run<R: Send>(&self, work: impl FnOnce() -> R + Send) -> R {
        self.0.lock().unwrap().on_main += 1;
        let result = work();
        self.0.lock().unwrap().on_main -= 1;
        result
    }

    fn is_current(&self) -> bool {
        self.0.lock().unwrap().caller_is_main
    }
}

impl SecureInput for Fake {
    fn is_active(&self) -> bool {
        self.0.lock().unwrap().secure
    }
}
