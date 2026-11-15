use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

type Action = Box<dyn FnOnce() + Send>;

/// The lock is held while the action runs, so dropping the watch waits for it to finish.
type Slot = Arc<Mutex<Option<Action>>>;

#[derive(Default)]
struct Shared {
    cancelled: AtomicBool,
    actions: Mutex<Vec<Slot>>,
}

/// Clones share one flag; work polls it or registers with [`Self::on_cancel`].
#[derive(Clone, Default)]
pub struct CancelToken(Arc<Shared>);

impl std::fmt::Debug for CancelToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("CancelToken")
            .field(&self.is_cancelled())
            .finish()
    }
}

/// Dropping it removes the action and waits if it is running, so an action must not drop
/// its own watch.
pub struct CancelWatch {
    token: CancelToken,
    slot: Slot,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    /// Irreversible. Every action runs even if one panics; the first panic is resumed afterwards.
    pub fn cancel(&self) {
        let slots = {
            let mut actions = lock(&self.0.actions);
            self.0.cancelled.store(true, Ordering::SeqCst);
            std::mem::take(&mut *actions)
        };
        let mut first_panic = None;
        for slot in slots {
            let mut action = lock(&slot);
            if let Some(run) = action.take()
                && let Err(payload) = panic::catch_unwind(AssertUnwindSafe(run))
            {
                first_panic.get_or_insert(payload);
            }
        }
        if let Some(payload) = first_panic {
            panic::resume_unwind(payload);
        }
    }

    /// Whether `other` is this token or a clone of it.
    pub fn is_same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::SeqCst)
    }

    /// Runs at once if already cancelled; at most once, and never after the watch is dropped.
    pub fn on_cancel(&self, action: impl FnOnce() + Send + 'static) -> CancelWatch {
        let slot = Slot::default();
        let mut actions = lock(&self.0.actions);
        if self.is_cancelled() {
            drop(actions);
            action();
        } else {
            *lock(&slot) = Some(Box::new(action));
            actions.push(Arc::clone(&slot));
        }
        CancelWatch {
            token: self.clone(),
            slot,
        }
    }
}

impl Drop for CancelWatch {
    fn drop(&mut self) {
        lock(&self.token.0.actions).retain(|slot| !Arc::ptr_eq(slot, &self.slot));
        // Waits here while the action runs; dropped outside the lock.
        let action = lock(&self.slot).take();
        drop(action);
    }
}
