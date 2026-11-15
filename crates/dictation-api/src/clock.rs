/// Lets tests control timestamps and durations.
pub trait Clock: Send + Sync {
    /// The current wall-clock time in milliseconds since the Unix epoch.
    fn now_ms(&self) -> u64;

    /// A timestamp in milliseconds that never goes backwards, for measuring durations. It
    /// has no relation to the wall clock.
    fn monotonic_ms(&self) -> u64;
}
