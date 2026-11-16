//! When the clipboard is put back after a paste: the rule alone, apart from any clock or thread.

use std::time::{Duration, Instant};

use crate::config::InsertionTuning;
use crate::insertion_parts::OfferNews;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitStep {
    Done,
    /// Look again after this long, or sooner on news.
    Wait(Duration),
}

/// The next step of the wait that began at `started`, when the paste keys went out or failed
/// to. Reads keep it going, but never past the longest wait for a paste. Once the text has been
/// read, another paste waiting for its turn ends it at once; before that, ending it would let
/// the application read the next paste's text or the clipboard put back instead.
pub fn wait_step(
    tuning: &InsertionTuning,
    news: &OfferNews,
    started: Instant,
    now: Instant,
    keys_sent: bool,
) -> WaitStep {
    if news.replaced || (news.superseded && news.last_read.is_some()) {
        return WaitStep::Done;
    }
    let ms = Duration::from_millis;
    let longest = started + ms(tuning.restore_unread_timeout_ms);
    let deadline = match news.last_read {
        Some(read) => (read + ms(tuning.restore_quiet_after_read_ms)).min(longest),
        None if keys_sent => longest,
        None => started + ms(tuning.restore_after_failed_paste_ms),
    };
    match deadline.checked_duration_since(now) {
        Some(left) if !left.is_zero() => WaitStep::Wait(left),
        _ => WaitStep::Done,
    }
}
