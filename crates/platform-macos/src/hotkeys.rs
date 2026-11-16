use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use dictation_api::{HotkeySource, ShortcutListener};
use dictation_types::{AppResult, HotkeyBinding, HotkeyNotice, ShortcutEvent, ShortcutId};

use crate::clock::now_ms;
use crate::delivery::{Delivery, Message};
use crate::input::{Disposition, InputEvent};
use crate::shortcut_matcher::ShortcutMatcher;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Notices are queued while the
/// matcher is locked, so they reach the listener in the order the matcher saw things, whichever
/// thread caused them.
pub(crate) struct Shared {
    matcher: Mutex<ShortcutMatcher>,
    /// Feeds the delivery thread while there is a listener.
    sender: Mutex<Option<Sender<Message>>>,
    /// Cleared before the listener is told watching
    /// stopped, so a listener that listens again on hearing it starts watching anew.
    watching: AtomicBool,
}

impl Shared {
    fn new(matcher: ShortcutMatcher) -> Self {
        Self {
            matcher: Mutex::new(matcher),
            sender: Mutex::new(None),
            watching: AtomicBool::new(false),
        }
    }

    fn set_watching(&self, watching: bool) {
        self.watching.store(watching, Ordering::Release);
    }

    fn is_watching(&self) -> bool {
        self.watching.load(Ordering::Acquire)
    }

    fn change<T>(&self, change: impl FnOnce(&mut ShortcutMatcher) -> T) -> T {
        let mut matcher = lock(&self.matcher);
        let result = change(&mut matcher);
        self.queue(
            matcher
                .take_pending()
                .into_iter()
                .map(HotkeyNotice::Shortcut),
        );
        result
    }

    pub(crate) fn handle(&self, input: InputEvent) -> Disposition {
        let mut matcher = lock(&self.matcher);
        let (events, disposition) = matcher.handle(input);
        self.queue(events.into_iter().map(HotkeyNotice::Shortcut));
        disposition
    }

    pub(crate) fn interrupt(&self, timestamp_ms: u64) {
        let mut matcher = lock(&self.matcher);
        let event: Option<ShortcutEvent> = matcher.interrupt(timestamp_ms);
        self.queue(event.map(HotkeyNotice::Shortcut));
    }

    /// Watching is marked as ended before the listener can hear it.
    pub(crate) fn watching_stopped(&self, timestamp_ms: u64) {
        let mut matcher = lock(&self.matcher);
        self.set_watching(false);
        let event = matcher.interrupt(timestamp_ms);
        self.queue(
            event
                .map(HotkeyNotice::Shortcut)
                .into_iter()
                .chain([HotkeyNotice::WatchingStopped]),
        );
    }

    /// Without a listener the notices are dropped.
    fn queue(&self, notices: impl IntoIterator<Item = HotkeyNotice>) {
        if let Some(sender) = lock(&self.sender).as_ref() {
            for notice in notices {
                // Fails only once the delivery thread has ended, when nobody is listening.
                let _ = sender.send(Message::Notice(notice));
            }
        }
    }
}

/// The parts that listening starts and stops, changed only together.
#[derive(Default)]
struct Control {
    #[cfg(target_os = "macos")]
    tap: Option<crate::event_tap::EventTap>,
    delivery: Option<Delivery>,
}

/// Global shortcuts, watched through an event tap that needs the Accessibility permission. The
/// keys of a shortcut with a key, and mouse side buttons, are kept from the application in front;
/// a lone modifier still reaches it. A modifier names its side.
pub struct MacosHotkeySource {
    shared: Arc<Shared>,
    /// Whether listening watches the keyboard; see [`Self::unwatched`].
    watch_system: bool,
    control: Mutex<Control>,
}

impl Default for MacosHotkeySource {
    fn default() -> Self {
        Self::new()
    }
}

impl MacosHotkeySource {
    pub fn new() -> Self {
        #[cfg(target_os = "macos")]
        let matcher = ShortcutMatcher::with_key_state(Box::new(crate::key_state::SystemKeyState));
        #[cfg(not(target_os = "macos"))]
        let matcher = ShortcutMatcher::new();
        Self::with_matcher(matcher, true)
    }

    /// A source that never watches the system: events come in only through [`Self::handle_input`],
    /// and it counts as watching from listening until stopped, as a tap would. For tests.
    pub fn unwatched() -> Self {
        Self::with_matcher(ShortcutMatcher::new(), false)
    }

    fn with_matcher(matcher: ShortcutMatcher, watch_system: bool) -> Self {
        Self {
            shared: Arc::new(Shared::new(matcher)),
            watch_system,
            control: Mutex::new(Control::default()),
        }
    }

    /// Public so it can be tested without the system.
    pub fn handle_input(&self, input: InputEvent) -> Disposition {
        self.shared.handle(input)
    }

    /// Ends a press in progress and tells the listener watching ended unasked, as when the system
    /// tears the tap down. Public so it can be tested without the system.
    pub fn watching_stopped(&self) {
        self.shared.watching_stopped(now_ms());
    }

    /// False after
    /// watching ended on its own, from before the listener hears it until listening again.
    pub fn is_listening(&self) -> bool {
        let control = lock(&self.control);
        control.delivery.is_some() && self.shared.is_watching()
    }

    /// Watching is marked as started first, so a tap the system tears down
    /// at once still ends up marked as ended.
    fn start_watching(&self, control: &mut Control) -> AppResult<()> {
        self.shared.set_watching(true);
        let started = if self.watch_system {
            self.start_tap(control)
        } else {
            Ok(())
        };
        if started.is_err() {
            self.shared.set_watching(false);
        }
        started
    }

    #[cfg(target_os = "macos")]
    fn start_tap(&self, control: &mut Control) -> AppResult<()> {
        control.tap = Some(crate::event_tap::EventTap::start(Arc::clone(&self.shared))?);
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    fn start_tap(&self, _control: &mut Control) -> AppResult<()> {
        Err(dictation_types::AppError::unsupported(
            "watching the keyboard this way works only on macOS",
        ))
    }

    /// Marks watching as ended, its tap already gone, and takes the delivery out, for the caller
    /// to finish once the control lock is released: the listener may be waiting for that lock.
    fn take_all(&self, control: &mut Control) -> Option<Delivery> {
        self.shared.set_watching(false);
        let delivery = control.delivery.take();
        *lock(&self.shared.sender) = None;
        delivery
    }
}

impl HotkeySource for MacosHotkeySource {
    fn register(&self, binding: HotkeyBinding) -> AppResult<()> {
        self.shared.change(|matcher| matcher.register(binding))
    }

    fn unregister(&self, id: &ShortcutId) -> AppResult<bool> {
        Ok(self.shared.change(|matcher| matcher.unregister(id)))
    }

    fn clear(&self) -> AppResult<()> {
        self.shared.change(ShortcutMatcher::clear);
        Ok(())
    }

    fn list_bindings(&self) -> AppResult<Vec<HotkeyBinding>> {
        Ok(lock(&self.shared.matcher).bindings())
    }

    fn listen(&self, listener: ShortcutListener) -> AppResult<()> {
        let mut control = lock(&self.control);
        let start = !self.shared.is_watching();
        if start {
            // A tap the system tore down is replaced.
            #[cfg(target_os = "macos")]
            drop(control.tap.take());
            // What was down before watching started is not known.
            self.shared.interrupt(now_ms());
        }
        if control.delivery.is_some() {
            if let Some(sender) = lock(&self.shared.sender).as_ref() {
                let _ = sender.send(Message::Replace(listener));
            }
        } else {
            let (delivery, sender) = Delivery::start(listener)?;
            *lock(&self.shared.sender) = Some(sender);
            control.delivery = Some(delivery);
        }
        if start && let Err(err) = self.start_watching(&mut control) {
            let delivery = self.take_all(&mut control);
            drop(control);
            if let Some(delivery) = delivery {
                delivery.finish();
            }
            return Err(err);
        }
        Ok(())
    }

    fn stop_listening(&self) -> AppResult<()> {
        let delivery = {
            let mut control = lock(&self.control);
            #[cfg(target_os = "macos")]
            drop(control.tap.take());
            // What is down once watching stops is no longer known.
            self.shared.interrupt(now_ms());
            self.take_all(&mut control)
        };
        if let Some(delivery) = delivery {
            delivery.finish();
        }
        Ok(())
    }
}

impl Drop for MacosHotkeySource {
    fn drop(&mut self) {
        let _ = self.stop_listening();
    }
}
