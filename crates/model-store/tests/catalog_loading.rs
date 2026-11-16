mod support;

use std::path::PathBuf;

use dictation_types::{ArchiveFormat, Language, ModelKind, PlatformTarget};
use model_store::{ModelCatalog, current_platform};
use serde_json::{Value, json};
use support::{catalog_json, id};

fn shipped_catalog_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../model-catalog.json")
}

fn entry(id: &str, platforms: &[&str]) -> Value {
    json!({
        "id": id,
        "name": format!("Name of {id}"),
        "kind": "asr",
        "engine": "engine-a",
        "group": "fast",
        "providedBySystem": false,
        "platforms": platforms,
        "languages": ["de", "pt-BR", "zh-Hant"],
        "files": [{
            "path": "weights/model.bin",
            "sizeBytes": 42,
            "sha256": "ab".repeat(32),
            "downloadUrls": ["https://example.invalid/a", "https://mirror.example.invalid/a"],
            "unpack": null
        }]
    })
}

#[test]
fn a_valid_catalog_loads_and_keeps_the_order_of_the_file() {
    let mut archive = entry("zeta", &["linux-x64"]);
    archive["files"][0]["unpack"] = json!("tar.bz2");
    archive["kind"] = json!("vad");
    let mut system = entry("alpha", &["macos-arm64"]);
    system["providedBySystem"] = json!(true);
    system["files"] = json!([]);
    system["kind"] = json!("llm");
    system["group"] = Value::Null;
    let text = catalog_json(vec![archive, system, entry("middle", &["windows-x64"])]);

    let catalog = ModelCatalog::from_json(&text).expect("valid catalog");
    let ids: Vec<&str> = catalog.entries().iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["zeta", "alpha", "middle"]);

    let zeta = &catalog.entries()[0];
    assert_eq!(zeta.kind, ModelKind::Vad);
    assert_eq!(zeta.name, "Name of zeta");
    assert_eq!(zeta.engine.as_str(), "engine-a");
    assert_eq!(zeta.group.as_deref(), Some("fast"));
    assert_eq!(zeta.supported_platforms, [PlatformTarget::LinuxX64]);
    assert_eq!(
        zeta.supported_languages,
        [
            Language::parse("de").unwrap(),
            Language::parse("pt-BR").unwrap(),
            Language::parse("zh-Hant").unwrap()
        ]
    );
    let file = &zeta.files[0];
    assert_eq!(file.path, "weights/model.bin");
    assert_eq!(file.size_bytes, 42);
    assert_eq!(file.sha256, "ab".repeat(32));
    assert_eq!(file.download_urls.len(), 2);
    assert_eq!(file.unpack, Some(ArchiveFormat::TarBz2));

    let alpha = &catalog.entries()[1];
    assert!(alpha.provided_by_system);
    assert!(alpha.files.is_empty());
    assert_eq!(alpha.kind, ModelKind::Llm);
    assert_eq!(alpha.group, None);
}

#[test]
fn a_catalog_with_no_models_is_empty() {
    let catalog = ModelCatalog::from_json(&catalog_json(Vec::new())).expect("valid");
    assert!(catalog.entries().is_empty());
}

#[test]
fn get_finds_an_entry_by_its_id() {
    let text = catalog_json(vec![
        entry("one", &["linux-x64"]),
        entry("two", &["linux-x64"]),
    ]);
    let catalog = ModelCatalog::from_json(&text).unwrap();
    assert_eq!(catalog.get(&id("two")).unwrap().name, "Name of two");
    assert_eq!(catalog.get(&id("one")).unwrap().name, "Name of one");
    assert!(catalog.get(&id("three")).is_none());
}

#[test]
fn for_platform_returns_only_the_entries_for_that_platform_in_order() {
    let text = catalog_json(vec![
        entry("c", &["linux-x64", "macos-arm64"]),
        entry("a", &["windows-x64"]),
        entry("b", &["macos-arm64"]),
    ]);
    let catalog = ModelCatalog::from_json(&text).unwrap();
    let names = |platform| -> Vec<String> {
        catalog
            .for_platform(platform)
            .iter()
            .map(|e| e.id.to_string())
            .collect()
    };
    assert_eq!(names(PlatformTarget::MacosArm64), ["c", "b"]);
    assert_eq!(names(PlatformTarget::WindowsX64), ["a"]);
    assert_eq!(names(PlatformTarget::LinuxX64), ["c"]);
    assert!(names(PlatformTarget::AndroidArm64).is_empty());
}

#[test]
fn the_shipped_catalog_loads_and_every_entry_is_valid() {
    let catalog = ModelCatalog::load(&shipped_catalog_path()).expect("the shipped catalog loads");
    assert!(
        !catalog.entries().is_empty(),
        "the shipped catalog lists models"
    );
}

#[test]
fn the_shipped_catalog_names_no_default_model() {
    let text = std::fs::read_to_string(shipped_catalog_path()).unwrap();
    let file: Value = serde_json::from_str(&text).unwrap();
    let top = file.as_object().expect("an object");
    assert_eq!(top.keys().collect::<Vec<_>>(), ["models"]);
    for model in file["models"].as_array().unwrap() {
        for key in model.as_object().unwrap().keys() {
            assert!(
                !key.to_ascii_lowercase().contains("default"),
                "the entry {} has a '{key}' field",
                model["id"]
            );
        }
    }
    // A catalog that tried to name one would not load.
    let mut with_default = file.clone();
    with_default["default"] = json!(file["models"][0]["id"]);
    assert!(ModelCatalog::from_json(&with_default.to_string()).is_err());
}

#[test]
fn loading_a_catalog_file_that_does_not_exist_fails() {
    let missing = support::TempDir::new("catalog").join("missing.json");
    assert!(ModelCatalog::load(&missing).is_err());
}

#[test]
fn the_current_platform_is_the_machine_running_the_tests() {
    let os = match std::env::consts::OS {
        "macos" => Some("macos"),
        "windows" => Some("windows"),
        "linux" => Some("linux"),
        "ios" => Some("ios"),
        "android" => Some("android"),
        _ => None,
    };
    let arch = match std::env::consts::ARCH {
        "aarch64" => Some("arm64"),
        "x86_64" => Some("x64"),
        _ => None,
    };
    let expected = os
        .zip(arch)
        .map(|(os, arch)| format!("{os}-{arch}"))
        .and_then(|name| name.parse::<PlatformTarget>().ok());
    assert_eq!(current_platform(), expected);
    if cfg!(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86_64"),
    )) {
        assert!(current_platform().is_some(), "a desktop platform is known");
    }
}
