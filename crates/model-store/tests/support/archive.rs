use std::io::Write;

use bzip2::Compression;
use bzip2::write::BzEncoder;
use tar::{Builder, EntryType, Header};

/// Written by hand so its name can be anything, including names no well-behaved tool writes.
pub struct RawEntry<'a> {
    pub name: &'a str,
    pub kind: EntryType,
    pub link: &'a str,
    pub data: &'a [u8],
}

fn header_for(entry: &RawEntry<'_>) -> Header {
    let mut header = Header::new_ustar();
    header.set_entry_type(entry.kind);
    header.set_size(entry.data.len() as u64);
    header.set_mode(if entry.kind == EntryType::Directory {
        0o755
    } else {
        0o644
    });
    header.set_mtime(0);
    let ustar = header.as_ustar_mut().expect("a ustar header");
    let name = entry.name.as_bytes();
    if name.len() <= ustar.name.len() {
        ustar.name[..name.len()].copy_from_slice(name);
    } else {
        // Long names are split into the prefix and the name at a '/'.
        let split = entry
            .name
            .char_indices()
            .filter(|(i, ch)| *ch == '/' && name.len() - i - 1 <= ustar.name.len())
            .map(|(i, _)| i)
            .next()
            .expect("a name that fits a ustar header");
        assert!(split <= ustar.prefix.len(), "the prefix fits");
        ustar.prefix[..split].copy_from_slice(&name[..split]);
        ustar.name[..name.len() - split - 1].copy_from_slice(&name[split + 1..]);
    }
    let link = entry.link.as_bytes();
    ustar.linkname[..link.len()].copy_from_slice(link);
    header.set_cksum();
    header
}

pub fn tar_bz2(entries: &[RawEntry<'_>]) -> Vec<u8> {
    let mut builder = Builder::new(BzEncoder::new(Vec::new(), Compression::fast()));
    for entry in entries {
        builder
            .append(&header_for(entry), entry.data)
            .expect("append an entry");
    }
    let mut encoder = builder.into_inner().expect("finish the archive");
    encoder.flush().unwrap();
    encoder.finish().expect("finish compressing")
}

pub fn file<'a>(name: &'a str, data: &'a [u8]) -> RawEntry<'a> {
    RawEntry {
        name,
        kind: EntryType::Regular,
        link: "",
        data,
    }
}

pub fn folder(name: &str) -> RawEntry<'_> {
    RawEntry {
        name,
        kind: EntryType::Directory,
        link: "",
        data: b"",
    }
}

/// The record `key=value` of a pax header, with its length in front as the format wants.
pub fn pax_record(key: &str, value: &str) -> Vec<u8> {
    let rest = format!(" {key}={value}\n");
    let mut len = rest.len() + 1;
    while format!("{len}{rest}").len() != len {
        len += 1;
    }
    format!("{len}{rest}").into_bytes()
}

pub fn pax_global(records: &[u8]) -> RawEntry<'_> {
    RawEntry {
        name: "pax_global_header",
        kind: EntryType::XGlobalHeader,
        link: "",
        data: records,
    }
}
