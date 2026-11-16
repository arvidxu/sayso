//! Runs against the real system, without sending keys or touching the user's clipboard.

#![cfg(target_os = "macos")]

use dictation_api::PermissionProvider;
use dictation_types::{ErrorKind, PermissionKind};
use platform_macos::clipboard::SystemClipboard;
use std::sync::Arc;

use platform_macos::{
    Clipboard, ClipboardContents, ClipboardSnapshot, MacosPermissions, TextOffer,
};

/// A clipboard of its own for one test, dropped at the end.
struct Scratch(SystemClipboard);

impl Scratch {
    fn new(test: &str) -> Self {
        Self(SystemClipboard::named(format!(
            "platform-macos-test-{test}-{}",
            std::process::id()
        )))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        self.0.release();
    }
}

const PLAIN_TEXT: &str = "public.utf8-plain-text";
const TRANSIENT: &str = "org.nspasteboard.TransientType";

fn read(clipboard: &SystemClipboard) -> ClipboardContents {
    match clipboard.contents().unwrap() {
        ClipboardSnapshot::Contents(contents) => contents,
        other => panic!("a clipboard of our own is always wholly readable: {other:?}"),
    }
}

fn types_of(contents: &ClipboardContents) -> Vec<Vec<String>> {
    contents
        .items
        .iter()
        .map(|item| item.iter().map(|(kind, _)| kind.clone()).collect())
        .collect()
}

#[test]
fn text_written_reads_back_and_is_marked_transient_only_when_asked() {
    let scratch = Scratch::new("text");
    let clipboard = &scratch.0;
    let text = "Grüße, 你好 🎙️";
    let count = clipboard.write_text(text, true).unwrap();
    assert_eq!(clipboard.change_count().unwrap(), count);
    let contents = read(clipboard);
    assert_eq!(contents.items.len(), 1);
    let item = &contents.items[0];
    let plain = item
        .iter()
        .find(|(kind, _)| kind == PLAIN_TEXT)
        .expect("plain text");
    assert_eq!(plain.1, text.as_bytes());
    assert!(item.iter().any(|(kind, _)| kind == TRANSIENT));

    clipboard.write_text(text, false).unwrap();
    let contents = read(clipboard);
    assert!(!contents.items[0].iter().any(|(kind, _)| kind == TRANSIENT));
}

#[test]
fn several_items_of_several_types_are_put_back_exactly() {
    let scratch = Scratch::new("restore");
    let clipboard = &scratch.0;
    let earlier = ClipboardContents {
        items: vec![
            vec![(
                "public.png".to_owned(),
                vec![0x89, 0x50, 0x4E, 0x47, 0, 1, 2],
            )],
            vec![
                (PLAIN_TEXT.to_owned(), b"caption".to_vec()),
                ("public.html".to_owned(), b"<b>caption</b>".to_vec()),
            ],
        ],
    };
    clipboard.restore(&earlier).unwrap();
    clipboard.write_text("dictated", true).unwrap();
    clipboard.restore(&earlier).unwrap();
    let back = read(clipboard);
    assert_eq!(types_of(&back), types_of(&earlier));
    assert_eq!(back, earlier);
}

#[test]
fn promised_text_is_handed_over_when_read_and_only_reads_of_the_text_count() {
    // Read from a test thread, the system answers in place and logs that it would rather be
    // asked on the main thread. Hearing of a new owner needs the main run loop, which tests lack.
    let scratch = Scratch::new("promise");
    let clipboard = &scratch.0;
    let offer = Arc::new(TextOffer::new());
    let count = clipboard.offer_text("Grüße 🎙️", &offer).unwrap();
    assert_eq!(clipboard.change_count().unwrap(), count);
    offer.start_counting();
    let contents = read(clipboard);
    assert_eq!(types_of(&contents), [[PLAIN_TEXT, TRANSIENT]]);
    assert_eq!(contents.items[0][0].1, "Grüße 🎙️".as_bytes());
    assert!(contents.items[0][1].1.is_empty());
    let news = offer.news();
    assert!(news.last_read.is_some());
    assert!(!news.replaced);
}

#[test]
fn the_report_lists_the_microphone_and_accessibility() {
    let report = MacosPermissions.permissions().unwrap();
    let kinds: Vec<PermissionKind> = report.permissions.iter().map(|(kind, _)| *kind).collect();
    assert_eq!(
        kinds,
        [PermissionKind::Microphone, PermissionKind::Accessibility]
    );
}

#[test]
fn input_monitoring_is_not_asked_for() {
    let err = MacosPermissions
        .request(PermissionKind::InputMonitoring)
        .unwrap_err();
    assert_eq!(err.kind, ErrorKind::Validation);
    let err = MacosPermissions
        .open_settings(PermissionKind::InputMonitoring)
        .unwrap_err();
    assert_eq!(err.kind, ErrorKind::Validation);
}
