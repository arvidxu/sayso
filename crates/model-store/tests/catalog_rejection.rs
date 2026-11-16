use dictation_types::ErrorKind;
use model_store::ModelCatalog;
use serde_json::{Value, json};

fn good_entry(id: &str) -> Value {
    json!({
        "id": id,
        "name": "A model",
        "kind": "asr",
        "engine": "engine-a",
        "group": null,
        "providedBySystem": false,
        "platforms": ["linux-x64", "macos-arm64"],
        "languages": ["en", "sw"],
        "files": [{
            "path": "model.bin",
            "sizeBytes": 10,
            "sha256": "0123456789abcdef".repeat(4),
            "downloadUrls": ["https://example.invalid/model.bin"],
            "unpack": null
        }]
    })
}

fn good_file() -> Value {
    json!({ "models": [good_entry("good-model")] })
}

fn changed(change: impl FnOnce(&mut Value)) -> String {
    let mut file = good_file();
    change(&mut file);
    file.to_string()
}

fn assert_refused(what: &str, change: impl FnOnce(&mut Value)) {
    match ModelCatalog::from_json(&changed(change)) {
        Ok(_) => panic!("a catalog with {what} was accepted"),
        Err(err) => assert_eq!(err.kind, ErrorKind::Validation, "{what}: {err}"),
    }
}

fn entry(file: &mut Value) -> &mut Value {
    &mut file["models"][0]
}

fn first_file(file: &mut Value) -> &mut Value {
    &mut file["models"][0]["files"][0]
}

#[test]
fn the_unchanged_catalog_is_accepted() {
    ModelCatalog::from_json(&good_file().to_string()).expect("valid");
}

#[test]
fn unknown_fields_are_refused_at_every_level() {
    assert_refused("a default model", |f| f["default"] = json!("good-model"));
    assert_refused("an unknown top-level field", |f| f["version"] = json!(2));
    assert_refused("a default flag on an entry", |f| {
        entry(f)["default"] = json!(true)
    });
    assert_refused("an unknown entry field", |f| {
        entry(f)["license"] = json!("x")
    });
    assert_refused("an unknown file field", |f| {
        first_file(f)["mirror"] = json!("x")
    });
}

#[test]
fn missing_fields_are_refused() {
    for key in ["id", "name", "kind", "engine", "platforms", "files"] {
        assert_refused(&format!("no {key}"), |f| {
            entry(f).as_object_mut().unwrap().remove(key);
        });
    }
    for key in ["path", "sizeBytes", "sha256", "downloadUrls"] {
        assert_refused(&format!("a file without {key}"), |f| {
            first_file(f).as_object_mut().unwrap().remove(key);
        });
    }
}

#[test]
fn unknown_kinds_platforms_languages_and_archive_formats_are_refused() {
    assert_refused("an unknown kind", |f| entry(f)["kind"] = json!("speaker"));
    assert_refused("an empty kind", |f| entry(f)["kind"] = json!(""));
    assert_refused("an unknown platform", |f| {
        entry(f)["platforms"] = json!(["linux-x64", "amiga-m68k"])
    });
    for tag in ["", "e", "123", "english language", "en--US"] {
        assert_refused(&format!("the language tag '{tag}'"), |f| {
            entry(f)["languages"] = json!(["en", tag])
        });
    }
    for format in ["zip", "tar.gz", ""] {
        assert_refused(&format!("the archive format '{format}'"), |f| {
            first_file(f)["unpack"] = json!(format)
        });
    }
}

#[test]
fn empty_ids_names_and_engines_are_refused() {
    for blank in ["", "   "] {
        assert_refused("an empty id", |f| entry(f)["id"] = json!(blank));
        assert_refused("an empty name", |f| entry(f)["name"] = json!(blank));
        assert_refused("an empty engine", |f| entry(f)["engine"] = json!(blank));
    }
}

#[test]
fn platform_lists_have_to_name_each_platform_once() {
    assert_refused("no platforms", |f| entry(f)["platforms"] = json!([]));
    assert_refused("a platform twice", |f| {
        entry(f)["platforms"] = json!(["linux-x64", "macos-arm64", "linux-x64"])
    });
}

#[test]
fn downloadable_models_need_files_and_system_models_must_have_none() {
    assert_refused("a downloadable model without files", |f| {
        entry(f)["files"] = json!([])
    });
    assert_refused("a system-provided model with files", |f| {
        entry(f)["providedBySystem"] = json!(true)
    });
}

#[test]
fn checksums_have_to_be_sha256_in_hexadecimal() {
    for sha in [
        String::new(),
        "abc".to_owned(),
        "0".repeat(63),
        "0".repeat(65),
        "g".repeat(64),
        format!("{} ", "0".repeat(63)),
    ] {
        assert_refused(&format!("the checksum '{sha}'"), |f| {
            first_file(f)["sha256"] = json!(sha)
        });
    }
}

#[test]
fn a_file_of_size_zero_is_refused() {
    assert_refused("a zero size", |f| first_file(f)["sizeBytes"] = json!(0));
    assert_refused("a negative size", |f| {
        first_file(f)["sizeBytes"] = json!(-1)
    });
}

#[test]
fn file_paths_that_leave_the_model_folder_are_refused() {
    for path in [
        "/model.bin",
        "\\model.bin",
        "C:\\model.bin",
        "C:/model.bin",
        "~/model.bin",
        "..",
        "../model.bin",
        "weights/../../model.bin",
        "weights\\..\\..\\model.bin",
    ] {
        assert_refused(&format!("the path '{path}'"), |f| {
            first_file(f)["path"] = json!(path)
        });
    }
}

#[test]
fn file_paths_that_are_malformed_are_refused() {
    for path in [
        "",
        " ",
        " model.bin",
        "model.bin ",
        "weights//model.bin",
        "weights/",
        "model\n.bin",
        "model\0.bin",
    ] {
        assert_refused(&format!("the path {path:?}"), |f| {
            first_file(f)["path"] = json!(path)
        });
    }
}

#[test]
fn file_paths_made_of_dot_segments_are_refused() {
    for path in [".", "./", "weights/.", "./."] {
        assert_refused(&format!("the path {path:?}"), |f| {
            first_file(f)["path"] = json!(path)
        });
    }
}

#[test]
fn two_files_whose_paths_differ_only_in_letter_case_are_refused() {
    assert_refused("two files differing only in case", |f| {
        let mut other = first_file(f).clone();
        other["path"] = json!("MODEL.bin");
        entry(f)["files"].as_array_mut().unwrap().push(other);
    });
}

#[test]
fn files_need_a_well_formed_download_address() {
    assert_refused("a file with no address", |f| {
        first_file(f)["downloadUrls"] = json!([])
    });
    for url in [
        "",
        "model.bin",
        "https://",
        "://example.invalid/model.bin",
        "https://example.invalid/my model.bin",
    ] {
        assert_refused(&format!("the address '{url}'"), |f| {
            first_file(f)["downloadUrls"] = json!(["https://example.invalid/ok", url])
        });
    }
}

#[test]
fn two_ids_differing_only_in_letter_case_are_refused() {
    assert_refused("ids differing only in case", |f| {
        let mut other = good_entry("Good-Model");
        other["name"] = json!("Another model");
        f["models"].as_array_mut().unwrap().push(other);
    });
    assert_refused("the same id twice", |f| {
        f["models"]
            .as_array_mut()
            .unwrap()
            .push(good_entry("good-model"));
    });
}

#[test]
fn ids_that_cannot_be_folder_names_are_refused() {
    for id in [
        "my model", "a/b", "a\\b", ".hidden", ".", "..", "a:b", "a*b", "tab\tid", "model.", "con",
        "Nul.bin", "com1",
    ] {
        assert_refused(&format!("the id {id:?}"), |f| entry(f)["id"] = json!(id));
    }
}

#[test]
fn ids_made_of_letters_digits_dots_dashes_and_underscores_are_accepted() {
    for id in ["model_1.5-small", "A1", "x.y.z"] {
        let text = changed(|f| entry(f)["id"] = json!(id));
        ModelCatalog::from_json(&text).unwrap_or_else(|err| panic!("{id}: {err}"));
    }
}

#[test]
fn a_catalog_that_is_not_json_is_refused() {
    for text in ["", "{", "[]", "{\"models\": {}}"] {
        let err = ModelCatalog::from_json(text).expect_err(text);
        assert_eq!(err.kind, ErrorKind::Validation);
    }
}
