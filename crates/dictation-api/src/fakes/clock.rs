use std::sync::{Arc, Mutex};

use super::lock::lock_unpoisoned;
use crate::Clock;

/// Advances only when told to; the wall-clock and monotonic readings are the same.
#[derive(Debug, Clone, Default)]
pub struct FakeClock {
    now_ms: Arc<Mutex<u64>>,
}

impl FakeClock {
    pub fn new(initial_ms: u64) -> Self {
        Self {
            now_ms: Arc::new(Mutex::new(initial_ms)),
        }
    }

    pub fn advance_ms(&self, delta_ms: u64) {
        let mut now_ms = lock_unpoisoned(&self.now_ms);
        *now_ms = now_ms.saturating_add(delta_ms);
    }
}

impl Clock for FakeClock {
    fn now_ms(&self) -> u64 {
        *lock_unpoisoned(&self.now_ms)
    }

    fn monotonic_ms(&self) -> u64 {
        *lock_unpoisoned(&self.now_ms)
    }
}
