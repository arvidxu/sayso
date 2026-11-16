/// Each item with the data of each of its types.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClipboardContents {
    pub items: Vec<Vec<(String, Vec<u8>)>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardSnapshot {
    /// An empty clipboard included.
    Contents(ClipboardContents),
    /// Some items or types were refused and are left out. It is put back, and the outcome says the rest was lost.
    Partial(ClipboardContents),
    /// It holds something, and nothing of it could be read. Putting back what was read would
    /// empty it, so it is not put back.
    Unreadable,
}

/// Each type an item lists, with its data, or `None` when that data could not be read.
pub(crate) type ReadItem = Vec<(String, Option<Vec<u8>>)>;

impl ClipboardSnapshot {
    /// An item that lists no types, and a type
    /// whose data could not be had, are left out; only when that leaves nothing of a clipboard
    /// that held something is it unreadable.
    pub fn from_read(items: impl IntoIterator<Item = ReadItem>) -> Self {
        let mut contents = ClipboardContents::default();
        let mut left_out = false;
        for item in items {
            left_out |= item.is_empty();
            let mut kept = Vec::with_capacity(item.len());
            for (kind, data) in item {
                match data {
                    Some(data) => kept.push((kind, data)),
                    None => left_out = true,
                }
            }
            if !kept.is_empty() {
                contents.items.push(kept);
            }
        }
        match (left_out, contents.items.is_empty()) {
            (false, _) => Self::Contents(contents),
            (true, true) => Self::Unreadable,
            (true, false) => Self::Partial(contents),
        }
    }

    /// What can be put back: everything, or what could be read; `None` when nothing could.
    pub fn readable(&self) -> Option<&ClipboardContents> {
        match self {
            Self::Contents(contents) | Self::Partial(contents) => Some(contents),
            Self::Unreadable => None,
        }
    }
}
