mod support;

use dictation_types::{ErrorKind, FeatureDescriptor, FeatureId, FeatureMountPoint, Mode, ModeId};
use extension::Registry;
use support::*;

fn mode_ids(modes: &[&Mode]) -> Vec<String> {
    modes.iter().map(|m| m.id.as_str().to_owned()).collect()
}

fn feature_ids(features: &[&FeatureDescriptor]) -> Vec<String> {
    features.iter().map(|f| f.id.as_str().to_owned()).collect()
}

#[test]
fn a_registered_mode_can_be_found_and_listed() {
    let mut registry = Registry::new();
    let polish = llm_mode("polish", "Polish {text} for {scene}");
    let plain = plain_mode("plain");
    registry.register_mode(polish.clone()).unwrap();
    registry.register_mode(plain.clone()).unwrap();

    assert_eq!(mode_entry(&registry, "polish"), Some(&polish));
    assert_eq!(mode_entry(&registry, "plain"), Some(&plain));
    assert_eq!(mode_entry(&registry, "missing"), None);

    let mut listed = mode_ids(&registry.modes().list());
    listed.sort();
    assert_eq!(listed, ["plain", "polish"]);
}

#[test]
fn a_mode_that_uses_a_language_model_without_a_prompt_is_refused_and_changes_nothing() {
    let mut registry = Registry::new();
    for prompt in ["", " ", "\n\t"] {
        let err = registry
            .register_mode(llm_mode("no-prompt", prompt))
            .unwrap_err();
        assert_eq!(err.kind, ErrorKind::Validation, "{prompt:?}");
        assert!(
            registry.upsert_mode(llm_mode("no-prompt", prompt)).is_err(),
            "{prompt:?}"
        );
    }
    assert!(registry.modes().list().is_empty());
    assert_eq!(registry.mode_source(&ModeId::new("no-prompt")), None);

    // An invalid replacement leaves the existing mode untouched.
    let good = llm_mode("polish", "Polish {text}");
    registry.register_mode(good.clone()).unwrap();
    assert!(registry.upsert_mode(llm_mode("polish", "  ")).is_err());
    assert_eq!(mode_entry(&registry, "polish"), Some(&good));
}

#[test]
fn a_duplicate_mode_id_is_refused() {
    let mut registry = Registry::new();
    let first = llm_mode("polish", "First {text}");
    registry.register_mode(first.clone()).unwrap();
    let err = registry
        .register_mode(llm_mode("polish", "Second {text}"))
        .unwrap_err();
    assert_eq!(err.kind, ErrorKind::AlreadyExists);
    assert_eq!(mode_entry(&registry, "polish"), Some(&first));
    assert_eq!(registry.modes().list().len(), 1);
}

#[test]
fn an_explicit_replacement_takes_over_the_mode_id() {
    let mut registry = Registry::new();
    registry
        .register_mode(llm_mode("polish", "First {text}"))
        .unwrap();
    let second = llm_mode("polish", "Second {text} with {clipboard}");
    registry.upsert_mode(second.clone()).unwrap();
    assert_eq!(mode_entry(&registry, "polish"), Some(&second));
    assert_eq!(registry.modes().list().len(), 1);

    let fresh = plain_mode("fresh");
    registry.upsert_mode(fresh.clone()).unwrap();
    assert_eq!(mode_entry(&registry, "fresh"), Some(&fresh));
}

#[test]
fn removing_a_mode_returns_it() {
    let mut registry = Registry::new();
    let polish = llm_mode("polish", "Polish {text}");
    registry.register_mode(polish.clone()).unwrap();
    registry.register_mode(plain_mode("plain")).unwrap();

    assert_eq!(
        registry.unregister_mode(&ModeId::new("polish")),
        Some(polish)
    );
    assert_eq!(registry.unregister_mode(&ModeId::new("polish")), None);
    assert_eq!(registry.unregister_mode(&ModeId::new("never")), None);
    assert_eq!(mode_ids(&registry.modes().list()), ["plain"]);

    registry.register_mode(plain_mode("polish")).unwrap();
    assert_eq!(registry.modes().list().len(), 2);
}

#[test]
fn features_are_listed_by_order_then_id() {
    let mut registry = Registry::new();
    for (id, order) in [
        ("delta", 20),
        ("alpha", 20),
        ("omega", -5),
        ("beta", 0),
        ("gamma", 20),
        ("zeta", 0),
        ("epsilon", 100),
    ] {
        registry
            .register_feature(feature(id, FeatureMountPoint::NavigationRoute, order))
            .unwrap();
    }
    assert_eq!(
        feature_ids(&registry.features().list()),
        [
            "omega", "beta", "zeta", "alpha", "delta", "gamma", "epsilon"
        ]
    );
}

#[test]
fn features_are_filtered_by_mount_point_and_stay_ordered() {
    let mut registry = Registry::new();
    for (id, mount, order) in [
        ("nav-late", FeatureMountPoint::NavigationRoute, 30),
        ("settings-b", FeatureMountPoint::SettingsSection, 5),
        ("nav-early", FeatureMountPoint::NavigationRoute, 10),
        ("tray", FeatureMountPoint::TrayMenuItem, 0),
        ("settings-a", FeatureMountPoint::SettingsSection, 5),
        ("nav-first", FeatureMountPoint::NavigationRoute, -1),
    ] {
        registry
            .register_feature(feature(id, mount, order))
            .unwrap();
    }

    let at = |mount| feature_ids(&registry.features().list_by_mount_point(mount));
    assert_eq!(
        at(FeatureMountPoint::NavigationRoute),
        ["nav-first", "nav-early", "nav-late"]
    );
    assert_eq!(
        at(FeatureMountPoint::SettingsSection),
        ["settings-a", "settings-b"]
    );
    assert_eq!(at(FeatureMountPoint::TrayMenuItem), ["tray"]);
    assert!(at(FeatureMountPoint::FloatingPanel).is_empty());
    assert_eq!(registry.features().list().len(), 6);
}

#[test]
fn a_feature_without_an_id_or_a_title_is_refused() {
    let mut registry = Registry::new();
    for id in ["", "   "] {
        let err = registry.register_feature(panel(id)).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Validation, "{id:?}");
    }
    for title in ["", "  "] {
        let untitled = FeatureDescriptor::new(
            FeatureId::new("untitled"),
            title,
            FeatureMountPoint::FloatingPanel,
            true,
        );
        let err = registry.register_feature(untitled).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Validation, "{title:?}");
    }
    assert!(registry.features().list().is_empty());
}

#[test]
fn a_duplicate_feature_id_is_refused() {
    let mut registry = Registry::new();
    let first = feature("history", FeatureMountPoint::NavigationRoute, 1);
    registry.register_feature(first.clone()).unwrap();

    let err = registry
        .register_feature(feature("history", FeatureMountPoint::TrayMenuItem, 99))
        .unwrap_err();
    assert_eq!(err.kind, ErrorKind::AlreadyExists);
    assert!(
        registry
            .register_feature(feature(" history ", FeatureMountPoint::TrayMenuItem, 99))
            .is_err()
    );
    assert_eq!(feature_entry(&registry, "history"), Some(&first));
    assert_eq!(registry.features().list().len(), 1);
}

#[test]
fn removing_a_feature_returns_it() {
    let mut registry = Registry::new();
    let history = feature("history", FeatureMountPoint::NavigationRoute, 1);
    registry.register_feature(history.clone()).unwrap();
    registry.register_feature(panel("overlay")).unwrap();

    assert_eq!(
        registry.unregister_feature(&FeatureId::new("history")),
        Some(history)
    );
    assert_eq!(
        registry.unregister_feature(&FeatureId::new("history")),
        None
    );
    assert_eq!(registry.unregister_feature(&FeatureId::new("never")), None);
    assert_eq!(feature_ids(&registry.features().list()), ["overlay"]);

    registry
        .register_feature(feature("history", FeatureMountPoint::TrayMenuItem, 2))
        .unwrap();
    assert_eq!(registry.features().list().len(), 2);
}
