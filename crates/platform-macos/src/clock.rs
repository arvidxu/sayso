use std::sync::OnceLock;
use std::time::Instant;

/// Milliseconds since this package first read the clock. It only moves forward, which is all
/// shortcut timing needs: only the difference between two events means anything.
pub fn now_ms() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    let elapsed = START.get_or_init(Instant::now).elapsed().as_millis();
    u64::try_from(elapsed).unwrap_or(u64::MAX)
}
