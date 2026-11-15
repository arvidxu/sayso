mod support;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use dictation_types::{
    AppError, AppResult, ErrorKind, ExtensionId, FeatureDescriptor, Mode, ModeId, ProviderDetails,
};
use extension::{Extension, Registry};
use support::*;

#[test]
fn the_meddling_extension_really_changes_the_registry_when_it_succeeds() {
    // Guards the tests below: if `meddle` changed nothing, they would prove nothing.
    let mut registry = populated_registry();
    let before = observe(&registry);
    registry
        .load_extension(&extension("meddler", meddle))
        .unwrap();

    assert_ne!(observe(&registry), before);
    assert_eq!(
        said_by(&registry, "built-in-speech").as_deref(),
        Some("교체됨")
    );
    assert_eq!(said_by(&registry, "earlier-speech"), None);
    assert!(feature_entry(&registry, "built-in-feature").is_none());
    assert!(registry.has_extension("meddler"));
}

#[test]
fn a_load_that_fails_or_panics_leaves_the_registry_exactly_as_it_was_and_usable() {
    for panics in [false, true] {
        let mut registry = populated_registry();
        let before = observe(&registry);
        let failing = extension("meddler", move |registry| {
            meddle(registry)?;
            if panics {
                panic!("the add-on hit a bug while registering");
            }
            Err(AppError::unsupported("this platform is not supported"))
        });
        assert!(
            registry.load_extension(&failing).is_err(),
            "panics: {panics}"
        );

        assert_eq!(observe(&registry), before, "panics: {panics}");
        assert!(!registry.has_extension("meddler"), "panics: {panics}");
        assert_eq!(registry.loaded_extensions(), [ExtensionId::new("earlier")]);
        let providers = registry.providers();
        assert!(providers.get_asr(&pid("meddler-speech")).is_none());
        assert!(providers.get_llm(&pid("meddler-writer")).is_none());
        assert_eq!(registry.mode_source(&ModeId::new("meddler-mode")), None);

        // What is registered afterwards is built-in again, not the failed extension's.
        register_speech(&mut registry, "after-speech", "vi+th", "xin chào").unwrap();
        registry.register_mode(plain_mode("after-mode")).unwrap();
        registry.register_feature(panel("after-feature")).unwrap();
        assert_eq!(provider_source(&registry, "after-speech"), None);
        assert_eq!(mode_source(&registry, "after-mode"), None);
        assert_eq!(feature_source(&registry, "after-feature"), None);
    }
}

#[test]
fn a_failed_load_returns_the_extensions_own_error() {
    let mut registry = Registry::new();
    let failing = extension("failing", |_| {
        Err(AppError::new(
            ErrorKind::Custom("add-on.data-file-missing".to_owned()),
            "cannot start",
        ))
    });
    let err = registry.load_extension(&failing).unwrap_err();
    assert_eq!(
        err.kind,
        ErrorKind::Custom("add-on.data-file-missing".to_owned())
    );
    assert_eq!(err.message, "cannot start");
}

#[test]
fn a_load_that_panics_is_reported_as_an_error_not_as_a_panic() {
    let mut registry = Registry::new();
    let panicking = extension("panicking", |_| panic!("bug"));
    let outcome = catch_unwind(AssertUnwindSafe(|| registry.load_extension(&panicking)));
    let result = outcome.expect("the panic must not escape the registry");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("panicking"),
        "the error names the extension: {err}"
    );
}

#[test]
fn a_panic_while_reporting_the_extension_id_is_contained_too() {
    struct BrokenId;
    impl Extension for BrokenId {
        fn id(&self) -> &str {
            panic!("the add-on cannot even say who it is")
        }
        fn register(&self, registry: &mut Registry) -> AppResult<()> {
            registry.register_feature(panel("broken-id-feature"))
        }
    }

    let mut registry = registry_with_built_ins();
    let before = observe(&registry);
    let outcome = catch_unwind(AssertUnwindSafe(|| registry.load_extension(&BrokenId)));
    assert!(
        matches!(outcome, Ok(Err(_))),
        "a panicking extension must be reported as a failed load, not crash the caller"
    );
    assert_eq!(observe(&registry), before);
}

#[test]
fn an_extension_that_returned_an_error_can_be_retried_once_the_cause_is_gone() {
    let broken = AtomicBool::new(true);
    let flaky = extension("flaky", |registry| {
        register_speech(registry, "flaky-speech", "am+ti", "ሰላም")?;
        registry.register_mode(plain_mode("flaky-mode"))?;
        registry.register_feature(panel("flaky-feature"))?;
        if broken.load(Ordering::SeqCst) {
            return Err(AppError::model("model files are not installed yet"));
        }
        Ok(())
    });

    let mut registry = registry_with_built_ins();
    assert!(registry.load_extension(&flaky).is_err());
    assert!(registry.load_extension(&flaky).is_err(), "still broken");
    assert!(!registry.has_extension("flaky"));

    broken.store(false, Ordering::SeqCst);
    registry.load_extension(&flaky).unwrap();
    assert!(registry.has_extension("flaky"));
    assert_eq!(said_by(&registry, "flaky-speech").as_deref(), Some("ሰላም"));
    let owner = Some("flaky".to_owned());
    assert_eq!(provider_source(&registry, "flaky-speech"), owner);
    assert_eq!(mode_source(&registry, "flaky-mode"), owner);
    assert_eq!(feature_source(&registry, "flaky-feature"), owner);
}

/// Builds a look-alike registry in which its own entries were registered as built-ins, gets
/// it marked as "loading forger" through a nested load, and swaps it in.
fn forge(registry: &mut Registry) -> AppResult<()> {
    let mut forged = Registry::new();
    for d in registry.providers().list_descriptors() {
        if d.extension_id.is_none()
            && let Some(e) = registry.providers().get_asr(&d.id)
        {
            let details = ProviderDetails::new(d.display_name.clone(), d.requires_credential);
            forged.register_asr_provider(details, e)?;
        }
    }
    for f in registry.features().list() {
        if f.extension_id.is_none() {
            forged.register_feature(f.clone())?;
        }
    }
    register_speech(&mut forged, "sneaky-speech", "pt+it", "olá")?;
    forged.register_feature(panel("sneaky-feature"))?;
    forged.upsert_mode(llm_mode("built-in-mode", "send {clipboard} somewhere"))?;

    for id in registry.loaded_extensions() {
        let features: Vec<FeatureDescriptor> = registry
            .features()
            .list()
            .into_iter()
            .filter(|f| f.extension_id.as_ref() == Some(id))
            .cloned()
            .collect();
        let modes: Vec<Mode> = registry
            .modes()
            .list()
            .into_iter()
            .filter(|m| registry.mode_source(&m.id) == Some(id))
            .cloned()
            .collect();
        forged.load_extension(&extension(id.as_str(), move |r| {
            for f in &features {
                r.register_feature(f.clone())?;
            }
            for m in &modes {
                r.register_mode(m.clone())?;
            }
            Ok(())
        }))?;
    }

    // Take the registry out of a nested load, so that it stays marked as loading "forger".
    let slot = Mutex::new(None);
    let thief = extension("forger", |r| {
        *slot.lock().unwrap() = Some(std::mem::take(r));
        Ok(())
    });
    let _ = forged.load_extension(&thief);
    *registry = slot.lock().unwrap().take().expect("the marked registry");
    Ok(())
}

#[test]
fn a_look_alike_registry_swapped_in_by_an_extension_is_refused_and_undone() {
    let mut registry = populated_registry();

    let err = registry
        .load_extension(&extension("forger", forge))
        .expect_err("the forgery must be refused");
    assert_eq!(err.kind, ErrorKind::Validation);
    assert!(!registry.has_extension("forger"));

    assert_eq!(registry.loaded_extensions().len(), 1);
    assert!(registry.has_extension("earlier"));
    assert!(provider_entry(&registry, "sneaky-speech").is_none());
    assert!(feature_entry(&registry, "sneaky-feature").is_none());

    let built_in = mode_entry(&registry, "built-in-mode").expect("built-in mode");
    assert_eq!(built_in.name, "Mode built-in-mode");
    assert!(
        built_in.llm.is_none(),
        "the built-in mode must not have gained a prompt"
    );
    assert_eq!(mode_source(&registry, "built-in-mode"), None);
    assert_eq!(
        mode_source(&registry, "earlier-mode"),
        Some("earlier".to_owned())
    );
}
