//! No public constant in these packages may hold text; product data comes at run time.

use std::fs;
use std::path::{Path, PathBuf};

/// Sibling directories of this package.
const PACKAGES: [&str; 3] = ["dictation-types", "dictation-api", "extension"];

fn rust_sources(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
    {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_sources(&path, found);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push(path);
        }
    }
}

/// Text-typed public constants or statics outside comments; one holding a shipped
/// configuration file's text is exempt.
fn public_text_constants(source: &str) -> Vec<(usize, String)> {
    let mut offenders = Vec::new();
    let mut open_declaration: Option<usize> = None;
    for (index, line) in source.lines().enumerate() {
        let code = line.trim_start();
        if code.starts_with("//") {
            continue;
        }
        let rest = code
            .strip_prefix("pub ")
            .or_else(|| code.strip_prefix("pub(crate) "))
            .or_else(|| code.strip_prefix("pub(super) "));
        let declares = rest.is_some_and(|rest| {
            (rest.starts_with("const ") && !rest.starts_with("const fn "))
                || rest.starts_with("static ")
        });
        if declares {
            open_declaration = Some(index);
        }
        if let Some(start) = open_declaration {
            let is_text = code.contains('"') || code.contains("str") || code.contains("String");
            let is_shipped_file = code.contains("include_str!(") && code.contains(".conf.json\")");
            if is_shipped_file {
                if offenders.last().is_some_and(|(line, _)| *line == start + 1) {
                    offenders.pop();
                }
                open_declaration = None;
                continue;
            }
            if is_text && offenders.last().is_none_or(|(line, _)| *line != start + 1) {
                offenders.push((
                    start + 1,
                    source
                        .lines()
                        .nth(start)
                        .unwrap_or_default()
                        .trim()
                        .to_owned(),
                ));
            }
            if code.contains(';') {
                open_declaration = None;
            }
        }
    }
    offenders
}

#[test]
fn the_constant_scanner_catches_what_it_is_meant_to_catch() {
    let sample = concat!(
        "/// pub const IN_A_DOC_COMMENT: &str = \"ignored\";\n",
        "pub const ALL: [Self; 2] = [Self::A, Self::B];\n",
        "pub const fn helper() -> u8 { 1 }\n",
        "const PRIVATE: &str = \"private text is not public\";\n",
        "pub const NAMED: &str = \"some-identifier\";\n",
        "pub static OTHER: &'static str = \"another\";\n",
        "pub const SPLIT: [&str; 2] = [\n",
        "    \"one\",\n",
        "    \"two\",\n",
        "];\n",
        "pub(crate) const INNER: &str =\n",
        "    \"wrapped\";\n",
        "pub const SHIPPED: &str = include_str!(\"../some.conf.json\");\n",
        "pub const SHIPPED_SPLIT: &str =\n",
        "    include_str!(\"../other.conf.json\");\n",
        "pub const OTHER_FILE: &str = include_str!(\"../names.txt\");\n",
    );
    let lines: Vec<usize> = public_text_constants(sample)
        .into_iter()
        .map(|(line, _)| line)
        .collect();
    assert_eq!(lines, [5, 6, 7, 11, 16]);
}

#[test]
fn no_public_constant_or_static_holds_text() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut offenders = Vec::new();
    for package in PACKAGES {
        let src = crates.join(package).join("src");
        let mut files = Vec::new();
        rust_sources(&src, &mut files);
        assert!(!files.is_empty(), "no source files under {}", src.display());
        for file in &files {
            let source = fs::read_to_string(file).unwrap();
            for (line, text) in public_text_constants(&source) {
                offenders.push(format!("{}:{line}: {text}", file.display()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "public constants or statics holding text:\n{}",
        offenders.join("\n")
    );
}
