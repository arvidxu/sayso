mod support;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use dictation_types::{AppError, ErrorKind, ExtensionId, FeatureMountPoint, ModeId};
use extension::Registry;
use support::*;

#[test]
fn an_extension_adds_entries_of_every_kind_and_each_is_attributed_to_it() {
    let mut registry = Registry::new();
    registry.load_extension(&TranslationAddOn).unwrap();

    assert_eq!(
        said_by(&registry, "add-on-speech").as_deref(),
        Some("привет 안녕하세요 bonjour")
    );
    assert_eq!(
        written_by(&registry, "add-on-writer").as_deref(),
        Some("translated text")
    );
    let mode = mode_entry(&registry, "add-on-translate").unwrap();
    assert_eq!(mode.placeholders(), ["text", "target_language"]);
    let feature = feature_entry(&registry, "add-on-glossary").unwrap();
    assert_eq!(feature.mount_point, FeatureMountPoint::SettingsSection);

    assert!(registry.has_extension("translation-add-on"));
    assert_eq!(
        registry.loaded_extensions(),
        [ExtensionId::new("translation-add-on")]
    );

    let owner = Some("translation-add-on".to_owned());
    assert_eq!(provider_source(&registry, "add-on-speech"), owner);
    assert_eq!(provider_source(&registry, "add-on-writer"), owner);
    assert_eq!(feature_source(&registry, "add-on-glossary"), owner);
    assert_eq!(mode_source(&registry, "add-on-translate"), owner);
}

#[test]
fn built_ins_stay_attributed_to_nobody_when_extensions_are_loaded_around_them() {
    let mut registry = registry_with_built_ins();
    registry.load_extension(&TranslationAddOn).unwrap();
    register_speech(&mut registry, "late-built-in", "bn+en", "আমি office যাচ্ছি").unwrap();
    registry.register_mode(plain_mode("late-mode")).unwrap();
    registry.register_feature(panel("late-feature")).unwrap();

    for id in ["built-in-speech", "built-in-writer", "late-built-in"] {
        assert_eq!(provider_source(&registry, id), None, "{id}");
    }
    assert_eq!(mode_source(&registry, "built-in-mode"), None);
    assert_eq!(mode_source(&registry, "late-mode"), None);
    assert_eq!(feature_source(&registry, "built-in-feature"), None);
    assert_eq!(feature_source(&registry, "late-feature"), None);

    // A built-in registration cannot claim to come from an extension either.
    let mut claimed = panel("claimed-feature");
    claimed.extension_id = Some(ExtensionId::new("somebody"));
    registry.register_feature(claimed).unwrap();
    assert_eq!(feature_source(&registry, "claimed-feature"), None);
}

#[test]
fn an_extension_cannot_attribute_its_entries_to_another_extension() {
    let mut registry = Registry::new();
    registry
        .load_extension(&extension("victim", |_| Ok(())))
        .unwrap();

    let impostor = extension("impostor", |registry| {
        let mut claimed = panel("impostor-feature");
        claimed.extension_id = Some(ExtensionId::new("victim"));
        registry.register_feature(claimed)
    });
    registry.load_extension(&impostor).unwrap();

    let owner = Some("impostor".to_owned());
    assert_eq!(feature_source(&registry, "impostor-feature"), owner);
}

#[test]
fn replacing_a_built_in_from_an_extension_attributes_the_replacement_to_the_extension() {
    let mut registry = registry_with_built_ins();
    let replacer = extension("replacer", |registry| {
        upsert_speech(registry, "built-in-speech", "ta+si", "வணக்கம்")?;
        upsert_writer(registry, "built-in-writer", "replaced answer")?;
        registry.upsert_mode(llm_mode("built-in-mode", "Polish {text}"))
    });
    registry.load_extension(&replacer).unwrap();

    let owner = Some("replacer".to_owned());
    assert_eq!(provider_source(&registry, "built-in-speech"), owner);
    assert_eq!(provider_source(&registry, "built-in-writer"), owner);
    assert_eq!(mode_source(&registry, "built-in-mode"), owner);
    assert_eq!(
        said_by(&registry, "built-in-speech").as_deref(),
        Some("வணக்கம்")
    );
    assert_eq!(
        written_by(&registry, "built-in-writer").as_deref(),
        Some("replaced answer")
    );
}

#[test]
fn replacing_an_extensions_entry_as_a_built_in_attributes_it_to_nobody() {
    let mut registry = Registry::new();
    registry.load_extension(&TranslationAddOn).unwrap();

    upsert_speech(&mut registry, "add-on-speech", "el", "καλημέρα").unwrap();
    registry
        .upsert_mode(plain_mode("add-on-translate"))
        .unwrap();

    assert_eq!(provider_source(&registry, "add-on-speech"), None);
    assert_eq!(mode_source(&registry, "add-on-translate"), None);
    assert_eq!(
        provider_source(&registry, "add-on-writer"),
        Some("translation-add-on".to_owned())
    );
}

#[test]
fn a_removed_mode_has_no_source() {
    let mut registry = Registry::new();
    registry.load_extension(&TranslationAddOn).unwrap();
    assert!(
        registry
            .unregister_mode(&ModeId::new("add-on-translate"))
            .is_some()
    );
    assert_eq!(registry.mode_source(&ModeId::new("add-on-translate")), None);

    registry
        .register_mode(plain_mode("add-on-translate"))
        .unwrap();
    assert_eq!(mode_source(&registry, "add-on-translate"), None);
}

#[test]
fn a_refusal_the_extension_handled_leaves_the_existing_entry_untouched() {
    let mut registry = registry_with_built_ins();
    let coping = extension("coping", |registry| {
        let _ = register_speech(registry, "built-in-speech", "uk+pl", "добрий день");
        let _ = register_writer(registry, "built-in-writer", "other");
        let _ = registry.register_mode(llm_mode("built-in-mode", "Other {text}"));
        let _ = registry.register_feature(feature(
            "built-in-feature",
            FeatureMountPoint::TrayMenuItem,
            9,
        ));
        let _ = registry.register_mode(llm_mode("coping-invalid", "   "));
        Ok(())
    });
    registry.load_extension(&coping).unwrap();

    assert!(registry.has_extension("coping"));
    assert_eq!(
        said_by(&registry, "built-in-speech").as_deref(),
        Some("bom dia, buongiorno")
    );
    assert_eq!(
        written_by(&registry, "built-in-writer").as_deref(),
        Some("built-in answer")
    );
    assert_eq!(provider_source(&registry, "built-in-speech"), None);
    assert_eq!(provider_source(&registry, "built-in-writer"), None);
    assert_eq!(
        mode_entry(&registry, "built-in-mode"),
        Some(&plain_mode("built-in-mode"))
    );
    assert_eq!(mode_source(&registry, "built-in-mode"), None);
    assert_eq!(feature_source(&registry, "built-in-feature"), None);
    assert_eq!(
        feature_entry(&registry, "built-in-feature")
            .unwrap()
            .mount_point,
        FeatureMountPoint::FloatingPanel
    );
    assert!(mode_entry(&registry, "coping-invalid").is_none());
}

#[test]
fn the_same_extension_cannot_be_loaded_twice() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&calls);
    let once = extension("once", move |registry| {
        let n = counter.fetch_add(1, Ordering::SeqCst);
        registry.register_feature(panel(&format!("once-feature-{n}")))
    });

    let mut registry = Registry::new();
    registry.load_extension(&once).unwrap();
    let err = registry.load_extension(&once).unwrap_err();
    assert_eq!(err.kind, ErrorKind::AlreadyExists);

    assert_eq!(registry.loaded_extensions(), [ExtensionId::new("once")]);
    assert_eq!(
        registry.features().list().len(),
        1,
        "nothing was registered a second time"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "a refused load does not run the extension"
    );
}

#[test]
fn extension_ids_that_differ_only_by_letter_case_are_one_extension() {
    let mut registry = Registry::new();
    registry
        .load_extension(&extension("honest", |_| Ok(())))
        .unwrap();
    let err = registry
        .load_extension(&extension("Honest", |_| Ok(())))
        .expect_err("already loaded");
    assert_eq!(err.kind, ErrorKind::AlreadyExists);
    assert!(registry.has_extension("HONEST"));
}

#[test]
fn an_extension_needs_a_non_blank_id() {
    for id in ["", " ", "\t\n", "\u{3000}"] {
        let mut registry = registry_with_built_ins();
        let before = observe(&registry);
        let ran = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&ran);
        let nameless = extension(id, move |registry| {
            flag.store(true, Ordering::SeqCst);
            registry.register_feature(panel("nameless-feature"))
        });

        let err = registry.load_extension(&nameless).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Validation, "{id:?}");
        assert!(
            !ran.load(Ordering::SeqCst),
            "a nameless extension must not run"
        );
        assert!(feature_entry(&registry, "nameless-feature").is_none());
        assert_eq!(observe(&registry), before);
    }
}

#[test]
fn an_extension_id_is_known_without_its_surrounding_whitespace() {
    let mut registry = Registry::new();
    registry
        .load_extension(&extension("  padded-name ", |registry| {
            registry.register_feature(panel("padded"))
        }))
        .unwrap();
    assert!(registry.has_extension("padded-name"));
    assert!(registry.has_extension(" padded-name  "));
    assert!(!registry.has_extension("padded"));
    assert_eq!(
        registry.loaded_extensions(),
        [ExtensionId::new("padded-name")]
    );
    assert_eq!(
        feature_source(&registry, "padded"),
        Some("padded-name".to_owned())
    );
}

#[test]
fn loaded_extensions_are_listed_in_the_order_they_loaded() {
    let mut registry = Registry::new();
    for id in ["zeta", "alpha", "mid"] {
        registry.load_extension(&extension(id, |_| Ok(()))).unwrap();
    }
    let _ = registry.load_extension(&extension("failed", |_| Err(AppError::internal("boom"))));
    registry
        .load_extension(&extension("last", |_| Ok(())))
        .unwrap();

    let listed: Vec<&str> = registry
        .loaded_extensions()
        .iter()
        .map(ExtensionId::as_str)
        .collect();
    assert_eq!(listed, ["zeta", "alpha", "mid", "last"]);
    assert!(!registry.has_extension("failed"));
    assert!(!registry.has_extension("never-loaded"));
}

#[test]
fn an_extension_cannot_load_another_extension_under_its_own_load() {
    let mut registry = registry_with_built_ins();
    let outer = extension("outer", |registry| {
        let inner = extension("inner", |registry| {
            registry.register_feature(panel("inner-feature"))
        });
        let refused = registry.load_extension(&inner);
        assert!(refused.is_err(), "a nested load must be refused");
        registry.register_feature(panel("outer-feature"))
    });
    registry.load_extension(&outer).unwrap();

    assert!(registry.has_extension("outer"));
    assert!(!registry.has_extension("inner"));
    assert!(feature_entry(&registry, "inner-feature").is_none());
    assert_eq!(
        feature_source(&registry, "outer-feature"),
        Some("outer".to_owned())
    );
}
