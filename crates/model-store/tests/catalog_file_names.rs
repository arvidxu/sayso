use dictation_types::ErrorKind;
use model_store::ModelCatalog;
use serde_json::json;

fn refused(path: &str) -> bool {
    let file = json!({
        "models": [{
            "id": "m",
            "name": "A model",
            "kind": "asr",
            "engine": "engine-a",
            "group": null,
            "providedBySystem": false,
            "platforms": ["linux-x64"],
            "languages": ["en"],
            "files": [{
                "path": path,
                "sizeBytes": 10,
                "sha256": "ab".repeat(32),
                "downloadUrls": ["https://example.invalid/model.bin"],
                "unpack": null
            }]
        }]
    });
    match ModelCatalog::from_json(&file.to_string()) {
        Ok(_) => false,
        Err(err) => {
            assert_eq!(err.kind, ErrorKind::Validation, "{path:?}: {err}");
            true
        }
    }
}

fn assert_all_refused(paths: &[&str]) {
    let accepted: Vec<&&str> = paths.iter().filter(|path| !refused(path)).collect();
    assert!(accepted.is_empty(), "accepted: {accepted:?}");
}

#[test]
fn ordinary_paths_are_accepted() {
    for path in [
        "model.bin",
        "weights/model.onnx",
        "a-b_c.d/e f.txt",
        "console/x",
    ] {
        assert!(!refused(path), "{path:?} was refused");
    }
}

#[test]
fn backslashes_are_refused() {
    assert_all_refused(&["weights\\model.bin", "\\model.bin", "model.bin\\"]);
}

#[test]
fn reserved_device_names_are_refused_in_any_letter_case() {
    assert_all_refused(&[
        "CON",
        "con",
        "PRN",
        "AUX",
        "NUL",
        "nul",
        "COM1",
        "com9",
        "LPT1",
        "lpt9",
        "weights/Aux",
        "CON/model.bin",
    ]);
}

#[test]
fn names_ending_in_a_dot_or_a_space_are_refused() {
    assert_all_refused(&[
        "model.",
        "weights./model.bin",
        "weights /model.bin",
        "model.bin ",
    ]);
}

#[test]
fn characters_some_systems_cannot_store_are_refused() {
    assert_all_refused(&[
        "a<b.bin",
        "a>b.bin",
        "a:b.bin",
        "a\"b.bin",
        "a|b.bin",
        "a?b.bin",
        "a*b.bin",
        "weights/a*b",
    ]);
}
