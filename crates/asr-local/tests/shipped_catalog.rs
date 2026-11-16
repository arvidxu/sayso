use std::path::Path;

use asr_local::capabilities_of;
use dictation_types::ModelKind;
use model_store::ModelCatalog;

fn shipped_catalog() -> ModelCatalog {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../model-catalog.json");
    ModelCatalog::load(&path).expect("the shipped catalog is valid")
}

#[test]
fn every_shipped_speech_recognition_model_says_what_it_can_do() {
    let catalog = shipped_catalog();
    for entry in catalog.entries() {
        if entry.kind != ModelKind::Asr {
            continue;
        }
        capabilities_of(entry)
            .unwrap_or_else(|err| panic!("the entry {} is incomplete: {err}", entry.id));
    }
}

#[test]
fn shipped_models_of_other_kinds_are_not_taken_for_speech_recognition_models() {
    let catalog = shipped_catalog();
    for entry in catalog.entries() {
        if entry.kind != ModelKind::Asr {
            assert!(capabilities_of(entry).is_err(), "{}", entry.id);
        }
    }
}
