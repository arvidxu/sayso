use platform_macos::{ClipboardContents, ClipboardSnapshot};

fn read(kind: &str, data: &[u8]) -> (String, Option<Vec<u8>>) {
    (kind.to_owned(), Some(data.to_vec()))
}

fn refused(kind: &str) -> (String, Option<Vec<u8>>) {
    (kind.to_owned(), None)
}

fn kept(kind: &str, data: &[u8]) -> (String, Vec<u8>) {
    (kind.to_owned(), data.to_vec())
}

#[test]
fn an_empty_clipboard_is_read_as_empty() {
    assert_eq!(
        ClipboardSnapshot::from_read(Vec::new()),
        ClipboardSnapshot::Contents(ClipboardContents::default())
    );
}

#[test]
fn an_item_without_types_or_a_type_without_data_leaves_out_only_that() {
    let snapshot = ClipboardSnapshot::from_read(vec![
        Vec::new(),
        vec![
            read("public.utf8-plain-text", b"hi"),
            refused("public.html"),
        ],
        vec![refused("public.png")],
    ]);
    let expected = ClipboardContents {
        items: vec![vec![kept("public.utf8-plain-text", b"hi")]],
    };
    assert_eq!(snapshot, ClipboardSnapshot::Partial(expected.clone()));
    assert_eq!(snapshot.readable(), Some(&expected));
}

#[test]
fn nothing_read_of_a_clipboard_that_held_something_is_unreadable() {
    for items in [
        vec![Vec::new()],
        vec![vec![refused("public.png")], vec![refused("public.html")]],
    ] {
        let snapshot = ClipboardSnapshot::from_read(items);
        assert_eq!(snapshot, ClipboardSnapshot::Unreadable);
        assert_eq!(snapshot.readable(), None);
    }
}
