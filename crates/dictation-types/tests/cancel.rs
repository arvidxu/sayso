use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use dictation_types::CancelToken;

#[test]
fn a_waiting_action_runs_once_at_the_stop_and_not_after_its_watch_is_dropped() {
    let ran = Arc::new(AtomicUsize::new(0));
    let count = |ran: &Arc<AtomicUsize>| {
        let ran = Arc::clone(ran);
        move || {
            ran.fetch_add(1, Ordering::SeqCst);
        }
    };
    let token = CancelToken::new();
    let kept = token.on_cancel(count(&ran));
    let dropped = token.on_cancel(count(&ran));
    drop(dropped);
    // An action that uses the same token does not hang the stop.
    let same = token.clone();
    let inner = count(&ran);
    let _reentrant = token.on_cancel(move || drop(same.on_cancel(inner)));
    token.clone().cancel();
    token.cancel();
    assert_eq!(
        ran.load(Ordering::SeqCst),
        2,
        "the kept action and the one added during the stop, each once"
    );
    drop(kept);

    // Asked for after the stop: runs right away.
    let _late = token.on_cancel(count(&ran));
    assert_eq!(ran.load(Ordering::SeqCst), 3);
    assert!(token.is_cancelled() && token.is_same(&token.clone()));
    assert!(!token.is_same(&CancelToken::new()));
}
