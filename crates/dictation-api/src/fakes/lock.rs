//! Locking that keeps working after a test thread panicked while holding the lock.

use std::sync::{Mutex, MutexGuard};

pub(super) fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
