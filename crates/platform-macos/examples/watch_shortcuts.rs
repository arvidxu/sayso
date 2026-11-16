//! Prints what happens to a few shortcuts, for trying keys by hand. Needs the Accessibility
//! permission for the program it runs in.
//!
//! cargo run -p platform-macos --example watch_shortcuts -- [seconds]

#[cfg(target_os = "macos")]
fn main() {
    use std::time::Duration;

    use dictation_api::HotkeySource;
    use dictation_types::{
        HotkeyBinding, HotkeyNotice, HotkeyTriggerMode, KeyCode, ModifierKey, ShortcutId,
    };

    let seconds: u64 = std::env::args()
        .nth(1)
        .map(|arg| arg.parse().expect("seconds must be a whole number"))
        .unwrap_or(20);
    let source = platform_macos::MacosHotkeySource::new();
    let hold = HotkeyTriggerMode::HoldToTalk;
    let toggle = HotkeyTriggerMode::ToggleOnPress;
    let lone = |name: &str, modifier, mode| {
        HotkeyBinding::standalone_modifier(ShortcutId::new(name), modifier, mode)
    };
    for binding in [
        lone("fn", ModifierKey::Fn, hold),
        lone("right-option", ModifierKey::RightAlt, hold),
        lone("caps-lock", ModifierKey::CapsLock, toggle),
        lone("mouse-back", ModifierKey::MouseSideBack, hold),
        HotkeyBinding::combination(
            ShortcutId::new("control-option-space"),
            vec![ModifierKey::LeftControl, ModifierKey::LeftAlt],
            KeyCode::Space,
            hold,
        ),
    ] {
        source.register(binding).expect("the shortcut registers");
    }
    source
        .listen(Box::new(|notice| match notice {
            HotkeyNotice::Shortcut(event) => println!(
                "{:>8} ms  {}  {:?}",
                event.timestamp_ms,
                event.binding_id.as_str(),
                event.state
            ),
            HotkeyNotice::WatchingStopped => println!("watching stopped"),
        }))
        .expect("watching the keyboard needs the Accessibility permission");
    println!("Watching for {seconds} seconds; press the shortcuts.");
    std::thread::sleep(Duration::from_secs(seconds));
    source.stop_listening().expect("stops");
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("this example runs only on macOS");
}
