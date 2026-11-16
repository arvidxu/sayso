//! Fails on any number in source that is not registered below: tuning values belong in the
//! configuration file of the package they tune. It does not catch choices made without a number.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Numbers allowed anywhere: they count, index, or compare, and tune nothing.
const STRUCTURAL: &[&str] = &["0", "1", "2", "0.0", "1.0"];

/// (file, number, why it is a fact). A file of a crate is named relative to the crates
/// directory, a file of the app relative to the repository. A number is allowed only in the
/// file it is registered for.
const REGISTERED: &[(&str, &str, &str)] = &[
    (
        "audio/src/speech_scorer.rs",
        "16000",
        "sample rate the voice activity model was trained on",
    ),
    (
        "audio/src/speech_scorer.rs",
        "256",
        "frame length the voice activity model was trained on",
    ),
    ("audio/src/measure.rs", "1000", "milliseconds in a second"),
    ("audio/src/config.rs", "1000", "milliseconds in a second"),
    (
        "model-store/src/catalog.rs",
        "4",
        "length of the device names Windows keeps, such as COM1",
    ),
    (
        "model-store/src/layout.rs",
        "4",
        "number of parts in a trash folder name, which the same file writes",
    ),
    (
        "model-store/src/layout.rs",
        "3",
        "position of the model id among those parts",
    ),
    (
        "dictation-types/src/audio.rs",
        "1000",
        "milliseconds in a second",
    ),
    (
        "dictation-types/src/model.rs",
        "64",
        "length of a SHA-256 digest in hexadecimal",
    ),
    (
        "dictation-types/src/mode.rs",
        "7",
        "number of prompt variables in the list next to it",
    ),
    (
        "dictation-types/src/language.rs",
        "8",
        "longest subtag of a language tag in the standard",
    ),
    (
        "dictation-types/src/language.rs",
        "4",
        "length of a script subtag in the standard",
    ),
    (
        "apps/desktop/src-tauri/src/i18n.rs",
        "4",
        "length of a script subtag in the standard",
    ),
    (
        "apps/desktop/src-tauri/src/i18n.rs",
        "3",
        "number of languages in the list next to it",
    ),
    (
        "apps/desktop/src-tauri/src/updates.rs",
        "60",
        "seconds in a minute and minutes in an hour",
    ),
    (
        "dictation-types/src/language.rs",
        "3",
        "length of an extended language or numeric region subtag in the standard",
    ),
    (
        "dictation-types/src/hotkey.rs",
        "24",
        "highest function key keyboards define",
    ),
];

/// Reference tables, named like the registered files: data, not code. Every number in them
/// is a code point, a key code the system defines, or a table entry.
const DATA_FILES: &[&str] = &[
    "dictation-types/src/language_data.rs",
    "dictation-types/src/mark_data.rs",
    "dictation-types/src/punctuation_data.rs",
    "dictation-types/src/script_data.rs",
];

/// The folder of the desktop app's own Rust code, relative to the repository.
const APP_SOURCE: &str = "apps/desktop/src-tauri/src";

fn crates_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn repository_dir() -> PathBuf {
    crates_dir().parent().unwrap().to_path_buf()
}

/// `path` relative to the crates directory, or to the repository when it is outside the
/// crates, with forward slashes.
fn relative(path: &Path) -> String {
    path.strip_prefix(crates_dir())
        .or_else(|_| path.strip_prefix(repository_dir()))
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn located(file: &str) -> PathBuf {
    if file.starts_with(APP_SOURCE) {
        repository_dir().join(file)
    } else {
        crates_dir().join(file)
    }
}

fn source_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            source_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Source with comments, strings and character literals blanked out.
fn code_only(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut i = 0;
    while i < chars.len() {
        let (c, next) = (chars[i], chars.get(i + 1).copied());
        let raw_start = (c == 'r' || (c == 'b' && next == Some('r')))
            && (i == 0 || !(chars[i - 1].is_alphanumeric() || chars[i - 1] == '_'));
        if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            // Block comments nest.
            let mut depth = 0;
            while i < chars.len() {
                if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            out.push(' ');
        } else if raw_start && raw_string_len(&chars[i..]).is_some() {
            i += raw_string_len(&chars[i..]).unwrap_or(1);
            out.push(' ');
        } else if c == '"' {
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                i += if chars[i] == '\\' { 2 } else { 1 };
            }
            i += 1;
            out.push(' ');
        } else if c == '\'' && (chars.get(i + 2) == Some(&'\'') || next == Some('\\')) {
            i += 1;
            while i < chars.len() && chars[i] != '\'' {
                i += if chars[i] == '\\' { 2 } else { 1 };
            }
            i += 1;
            out.push(' ');
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

/// The length of the raw string literal (`r"…"`, `r#"…"#`, `br"…"`) at the start of `chars`,
/// or `None` when there is none.
fn raw_string_len(chars: &[char]) -> Option<usize> {
    let mut i = usize::from(chars.first() == Some(&'b'));
    if chars.get(i) != Some(&'r') {
        return None;
    }
    i += 1;
    let hashes = chars[i..].iter().take_while(|c| **c == '#').count();
    i += hashes;
    if chars.get(i) != Some(&'"') {
        return None;
    }
    i += 1;
    while i < chars.len() {
        if chars[i] == '"'
            && chars[i + 1..]
                .iter()
                .take(hashes)
                .filter(|c| **c == '#')
                .count()
                == hashes
        {
            return Some(i + 1 + hashes);
        }
        i += 1;
    }
    Some(chars.len())
}

/// The numbers written in `code`, without digit separators and type suffixes.
fn numbers(code: &str) -> Vec<String> {
    let chars: Vec<char> = code.chars().collect();
    let mut found = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let starts_number = chars[i].is_ascii_digit()
            && (i == 0 || !(chars[i - 1].is_alphanumeric() || chars[i - 1] == '_'));
        if !starts_number {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
            i += 1;
        }
        // A fraction, but not a range (`1..=3`) or a method call (`1.max(2)`).
        if i + 1 < chars.len() && chars[i] == '.' && chars[i + 1].is_ascii_digit() {
            i += 1;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
        }
        let mut text: String = chars[start..i].iter().filter(|c| **c != '_').collect();
        for suffix in [
            "usize", "u128", "u64", "u32", "u16", "u8", "i64", "i32", "f64", "f32",
        ] {
            if let Some(stripped) = text.strip_suffix(suffix) {
                text = stripped.to_owned();
                break;
            }
        }
        found.push(text);
    }
    found
}

#[test]
fn every_number_in_source_is_structural_or_a_registered_fact() {
    let mut files = Vec::new();
    for entry in fs::read_dir(crates_dir()).unwrap() {
        let src = entry.unwrap().path().join("src");
        if src.is_dir() {
            source_files(&src, &mut files);
        }
    }
    assert!(
        files.len() > 40,
        "the source files of the crates were not found"
    );
    let crate_files = files.len();
    source_files(&repository_dir().join(APP_SOURCE), &mut files);
    assert!(
        files.len() > crate_files,
        "the source files of the app were not found"
    );

    let mut problems = BTreeSet::new();
    for path in files {
        let name = relative(&path);
        if DATA_FILES.contains(&name.as_str()) {
            continue;
        }
        for number in numbers(&code_only(&fs::read_to_string(&path).unwrap())) {
            let allowed = STRUCTURAL.contains(&number.as_str())
                || REGISTERED
                    .iter()
                    .any(|(file, value, _)| name == *file && *value == number);
            if !allowed {
                problems.insert(format!("{name}: {number}"));
            }
        }
    }
    assert!(
        problems.is_empty(),
        "numbers in source that are neither configuration nor registered facts:\n{}",
        problems.into_iter().collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn every_registered_fact_says_why_and_is_still_in_use() {
    for (file, number, reason) in REGISTERED {
        assert!(
            reason.split_whitespace().count() >= 3,
            "{file} {number}: say why it is a fact"
        );
        let path = located(file);
        let used = fs::read_to_string(&path)
            .is_ok_and(|source| numbers(&code_only(&source)).iter().any(|n| n == number));
        assert!(
            used,
            "{file} no longer contains {number}: remove it from the list"
        );
    }
}

#[test]
fn the_scanner_sees_numbers_and_ignores_comments_and_text() {
    let code = code_only(
        "let a = 1_024; // 99 in a comment\nlet b = \"7 in text\"; let c = 'x'; let d = 0.75f32;",
    );
    assert_eq!(numbers(&code), ["1024", "0.75"]);
    assert_eq!(
        numbers("for i in 3..=5 { x.max(12) } let name2 = y3;"),
        ["3", "5", "12"]
    );
}

#[test]
fn the_scanner_ignores_block_comments_and_raw_strings_but_not_the_code_around_them() {
    let code = code_only(
        "let a = 5; /* 11 /* 12 */ 13 */ let b = r\"14\"; let c = r#\"15 \" 16\"#; let d = br\"17\"; let e = 6;",
    );
    assert_eq!(numbers(&code), ["5", "6"]);
    assert_eq!(numbers(&code_only("let r = 3; let br = 4;")), ["3", "4"]);
}

#[test]
fn a_number_registered_for_one_file_is_not_allowed_in_another_of_the_same_name() {
    let registered: Vec<&str> = REGISTERED.iter().map(|(file, _, _)| *file).collect();
    for file in &registered {
        assert!(
            file.contains("/src/"),
            "{file}: register numbers by their path inside the crates directory or the repository"
        );
    }
    assert!(!registered.contains(&"speech_scorer.rs"));
}
