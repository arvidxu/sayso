use configuration::{PackageConfig, ProductConfig};
use dictation_types::{AppResult, ErrorKind};
use model_store::{LocalCacheLocation, ModelsConfig};
use serde_json::{Value, json};

const PRODUCT_FILE: &str = include_str!("../../../product.conf.json");

fn product() -> Value {
    serde_json::from_str(PRODUCT_FILE).expect("the product configuration is JSON")
}

fn product_text(overrides: Value) -> String {
    let mut file = product();
    file["dictation"]["overrides"] = overrides;
    file.to_string()
}

fn read(product: &str, layers: &[&str]) -> AppResult<ModelsConfig> {
    ProductConfig::from_layers(product, layers)?.package()
}

fn with_overrides(overrides: Value) -> ModelsConfig {
    read(&product_text(overrides.clone()), &[])
        .unwrap_or_else(|err| panic!("the overrides {overrides} were refused: {err}"))
}

fn assert_refused_at(place: &str, overrides: Value) {
    match read(&product_text(overrides.clone()), &[]) {
        Ok(_) => panic!("the overrides {overrides} were accepted"),
        Err(err) => {
            assert_eq!(err.kind, ErrorKind::Validation, "{overrides}: {err}");
            assert!(err.message.contains(place), "{overrides}: {err}");
        }
    }
}

#[test]
fn where_to_look_for_files_other_tools_downloaded_is_the_products_decision() {
    assert_eq!(ModelsConfig::NAME, "model-store");
    assert!(
        ModelsConfig::bundled().unwrap().local_caches.is_empty(),
        "the model-store package looks nowhere by itself"
    );
    let file = product();
    let written = file["dictation"]["overrides"][ModelsConfig::NAME]["localCaches"]
        .as_array()
        .expect("the product file lists cache locations");
    let expected: Vec<LocalCacheLocation> = written
        .iter()
        .map(|cache| LocalCacheLocation {
            env_var: cache["envVar"].as_str().map(str::to_owned),
            sub_path: cache["subPath"].as_str().expect("a sub path").to_owned(),
            file_pattern: cache["filePattern"].as_str().expect("a pattern").to_owned(),
        })
        .collect();
    let shipped = read(PRODUCT_FILE, &[]).unwrap();
    assert!(!shipped.local_caches.is_empty());
    assert_eq!(shipped.local_caches, expected);
}

#[test]
fn without_overrides_the_package_runs_with_the_values_it_ships() {
    let bundled = ModelsConfig::bundled().unwrap();
    assert_eq!(with_overrides(json!({})), bundled);
    assert_eq!(with_overrides(json!({ "model-store": {} })), bundled);
}

#[test]
fn an_override_replaces_only_the_value_it_names() {
    let reference = with_overrides(json!({}));
    let mut expected = reference.clone();
    expected.download.retry_delay_ms = 5;
    expected.download.attempts_per_address = 9;
    assert_ne!(expected, reference);
    assert_eq!(
        with_overrides(
            json!({ "model-store": { "download": { "retryDelayMs": 5, "attemptsPerAddress": 9 } } })
        ),
        expected
    );
}

#[test]
fn a_list_in_a_layer_replaces_the_whole_list_the_product_file_gave() {
    let shipped = read(PRODUCT_FILE, &[]).unwrap();
    assert!(
        shipped.local_caches.len() > 1,
        "the product file gives a longer list"
    );
    let layer = json!({ "model-store": { "localCaches": [
        { "envVar": "SOME_CACHE", "subPath": "a/b", "filePattern": "*/{sha256}.bin" }
    ] } })
    .to_string();
    let mut expected = shipped.clone();
    expected.local_caches = vec![LocalCacheLocation {
        env_var: Some("SOME_CACHE".to_owned()),
        sub_path: "a/b".to_owned(),
        file_pattern: "*/{sha256}.bin".to_owned(),
    }];
    assert_eq!(read(PRODUCT_FILE, &[&layer]).unwrap(), expected);

    let none = json!({ "model-store": { "localCaches": [] } }).to_string();
    assert!(
        read(PRODUCT_FILE, &[&none])
            .unwrap()
            .local_caches
            .is_empty()
    );
}

#[test]
fn an_override_the_package_does_not_have_or_cannot_run_with_is_refused() {
    let cases = [
        (
            "dictation.overrides.model-store.download.retries",
            json!({ "model-store": { "download": { "retries": 3 } } }),
        ),
        (
            "dictation.overrides.model-store.localCaches",
            json!({ "model-store": { "localCaches": {} } }),
        ),
        (
            "dictation.overrides.model-store.download",
            json!({ "model-store": { "download": [] } }),
        ),
        (
            "dictation.overrides.model-store",
            json!({ "model-store": { "download": { "attemptsPerAddress": 0 } } }),
        ),
        (
            "dictation.overrides.model-store",
            json!({ "model-store": { "localCaches": ["a/b"] } }),
        ),
    ];
    for (place, overrides) in cases {
        assert_refused_at(place, overrides);
    }
}
