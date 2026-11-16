//! The main thread, reached through its dispatch queue. Its run loop or dispatch queue must be
//! served, as an application's is; a program whose main thread never serves them would wait
//! forever for the first paste.

use objc2::MainThreadMarker;

use crate::insertion::MainThread;

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemMainThread;

impl MainThread for SystemMainThread {
    fn run<R: Send>(&self, work: impl FnOnce() -> R + Send) -> R {
        dispatch2::run_on_main(|_| work())
    }

    fn is_current(&self) -> bool {
        MainThreadMarker::new().is_some()
    }
}
