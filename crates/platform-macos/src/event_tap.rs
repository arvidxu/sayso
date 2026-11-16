//! Watches keyboard and mouse through an event tap on its own thread. The callback only runs
//! the matcher and queues notices, so the system does not turn the tap off for being slow.

use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr::{NonNull, null_mut};
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};

use dictation_types::{AppError, AppResult, PermissionKind};
use objc2_core_foundation::{CFMachPort, CFRetained, CFRunLoop, kCFRunLoopCommonModes};
use objc2_core_graphics::{
    CGEvent, CGEventField, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventTapProxy, CGEventType,
};

use crate::clock::now_ms;
use crate::hotkeys::Shared;
use crate::input::{Disposition, InputEvent, InputKind};
use crate::keyboard::OWN_EVENT_MARK;

/// The events the tap asks for: keys, modifier changes, the extra mouse buttons, and the main
/// buttons and scrolling. The main buttons and scrolling always go through: a click, a drag or
/// a scroll interrupts a modifier down on its own, and one cannot go down while a main button
/// is held.
const EVENTS: u64 = (1 << CGEventType::KeyDown.0)
    | (1 << CGEventType::KeyUp.0)
    | (1 << CGEventType::FlagsChanged.0)
    | (1 << CGEventType::OtherMouseDown.0)
    | (1 << CGEventType::OtherMouseUp.0)
    | (1 << CGEventType::OtherMouseDragged.0)
    | (1 << CGEventType::LeftMouseDown.0)
    | (1 << CGEventType::LeftMouseUp.0)
    | (1 << CGEventType::LeftMouseDragged.0)
    | (1 << CGEventType::RightMouseDown.0)
    | (1 << CGEventType::RightMouseUp.0)
    | (1 << CGEventType::RightMouseDragged.0)
    | (1 << CGEventType::ScrollWheel.0);

struct Context {
    shared: Arc<Shared>,
    /// The tap, so the callback can turn it back on after the system turns it off.
    port: AtomicPtr<CFMachPort>,
}

/// The watcher thread's run loop, for stopping it from another thread.
struct RunLoopHandle(CFRetained<CFRunLoop>);

// SAFETY: run loop objects are thread-safe, and the handle is used only to stop the loop.
unsafe impl Send for RunLoopHandle {}

/// A running event tap. Dropping it stops the tap and waits for its thread to end. When the
/// system tears the tap down, the thread tells the listener and ends on its own.
pub(crate) struct EventTap {
    stop: Arc<AtomicBool>,
    run_loop: RunLoopHandle,
    thread: Option<JoinHandle<()>>,
}

impl EventTap {
    /// Fails when the system refuses it, which it does without the
    /// Accessibility permission.
    pub(crate) fn start(shared: Arc<Shared>) -> AppResult<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let (started_tx, started_rx) = mpsc::channel();
        let thread_stop = Arc::clone(&stop);
        let thread = thread::Builder::new()
            .name("shortcut-watcher".to_owned())
            .spawn(move || run(shared, thread_stop, started_tx))
            .map_err(|err| {
                AppError::hotkey(format!("could not start watching the keyboard: {err}"))
            })?;
        match started_rx.recv() {
            Ok(Ok(run_loop)) => Ok(Self {
                stop,
                run_loop,
                thread: Some(thread),
            }),
            Ok(Err(err)) => {
                let _ = thread.join();
                Err(err)
            }
            Err(_) => {
                let _ = thread.join();
                Err(AppError::hotkey(
                    "the keyboard watcher stopped before it started",
                ))
            }
        }
    }
}

impl Drop for EventTap {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let Some(thread) = self.thread.take() else {
            return;
        };
        // A stop that lands between the thread checking the flag and running its loop is lost,
        // so keep stopping until the thread has ended.
        while !thread.is_finished() {
            self.run_loop.0.stop();
            thread::yield_now();
        }
        let _ = thread.join();
    }
}

fn run(
    shared: Arc<Shared>,
    stop: Arc<AtomicBool>,
    started: mpsc::Sender<AppResult<RunLoopHandle>>,
) {
    let watcher = Arc::clone(&shared);
    let context = Box::into_raw(Box::new(Context {
        shared,
        port: AtomicPtr::new(null_mut()),
    }));
    // SAFETY: the callback matches the tap's signature; `context` outlives the tap.
    let port = unsafe {
        CGEvent::tap_create(
            CGEventTapLocation::SessionEventTap,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::Default,
            EVENTS,
            Some(on_event),
            context.cast(),
        )
    };
    let Some(port) = port else {
        // SAFETY: made above and never handed to a tap.
        drop(unsafe { Box::from_raw(context) });
        let _ = started.send(Err(AppError::permission_denied(
            Some(PermissionKind::Accessibility),
            "macOS did not let this application watch the keyboard; it needs the Accessibility permission",
        )));
        return;
    };
    // SAFETY: `context` is valid; the port it points to lives until the end of this function.
    unsafe { &*context }.port.store(
        std::ptr::from_ref::<CFMachPort>(&port).cast_mut(),
        Ordering::Release,
    );
    let attached = CFMachPort::new_run_loop_source(None, Some(&port), 0).zip(CFRunLoop::current());
    let Some((source, run_loop)) = attached else {
        port.invalidate();
        // SAFETY: the tap is invalidated, so the callback no longer runs.
        drop(unsafe { Box::from_raw(context) });
        let _ = started.send(Err(AppError::hotkey(
            "could not attach the keyboard watcher",
        )));
        return;
    };
    // SAFETY: a constant string the system defines.
    run_loop.add_source(Some(&source), unsafe { kCFRunLoopCommonModes });
    // A new tap is already enabled; events reach it once the loop runs.
    let _ = started.send(Ok(RunLoopHandle(run_loop.clone())));
    // The system invalidates the tap when it takes the permission away; the loop then has
    // nothing left to run.
    while !stop.load(Ordering::Acquire) && port.is_valid() {
        CFRunLoop::run();
    }
    source.invalidate();
    port.invalidate();
    // SAFETY: the tap is invalidated and its source removed, so the callback no longer runs.
    drop(unsafe { Box::from_raw(context) });
    if !stop.load(Ordering::Acquire) {
        // The system tore the tap down, as it does when the permission is taken away.
        watcher.watching_stopped(now_ms());
    }
}

fn read(event_type: CGEventType, event: &CGEvent) -> Option<InputEvent> {
    let field = |field| CGEvent::integer_value_field(Some(event), field);
    let key = || field(CGEventField::KeyboardEventKeycode) as u16;
    let kind = match event_type {
        CGEventType::KeyDown => InputKind::KeyDown {
            key: key(),
            repeat: field(CGEventField::KeyboardEventAutorepeat) != 0,
        },
        CGEventType::KeyUp => InputKind::KeyUp { key: key() },
        CGEventType::FlagsChanged => InputKind::FlagsChanged { key: key() },
        CGEventType::OtherMouseDown | CGEventType::LeftMouseDown | CGEventType::RightMouseDown => {
            InputKind::MouseDown {
                button: field(CGEventField::MouseEventButtonNumber),
            }
        }
        CGEventType::OtherMouseUp | CGEventType::LeftMouseUp | CGEventType::RightMouseUp => {
            InputKind::MouseUp {
                button: field(CGEventField::MouseEventButtonNumber),
            }
        }
        CGEventType::OtherMouseDragged
        | CGEventType::LeftMouseDragged
        | CGEventType::RightMouseDragged => InputKind::MouseDragged {
            button: field(CGEventField::MouseEventButtonNumber),
        },
        CGEventType::ScrollWheel => InputKind::Scroll,
        _ => return None,
    };
    Some(InputEvent::new(
        kind,
        CGEvent::flags(Some(event)).0,
        now_ms(),
    ))
}

unsafe extern "C-unwind" fn on_event(
    _proxy: CGEventTapProxy,
    event_type: CGEventType,
    event: NonNull<CGEvent>,
    user_info: *mut c_void,
) -> *mut CGEvent {
    let pass = event.as_ptr();
    // SAFETY: `user_info` is the context `run` made, alive while the tap is.
    let context = unsafe { &*user_info.cast::<Context>() };
    // SAFETY: the system hands a valid event for the duration of the call.
    let event = unsafe { event.as_ref() };
    // A panic must not cross into the system; the event then goes on as if unwatched.
    let disposition = catch_unwind(AssertUnwindSafe(|| {
        if event_type == CGEventType::TapDisabledByTimeout
            || event_type == CGEventType::TapDisabledByUserInput
        {
            let port = context.port.load(Ordering::Acquire);
            if !port.is_null() {
                // SAFETY: the port lives as long as the context.
                CGEvent::tap_enable(unsafe { &*port }, true);
            }
            context.shared.interrupt(now_ms());
            return Disposition::PassThrough;
        }
        if CGEvent::integer_value_field(Some(event), CGEventField::EventSourceUserData)
            == OWN_EVENT_MARK
        {
            return Disposition::PassThrough;
        }
        match read(event_type, event) {
            Some(input) => context.shared.handle(input),
            None => Disposition::PassThrough,
        }
    }));
    match disposition {
        Ok(Disposition::Swallow) => null_mut(),
        _ => pass,
    }
}
