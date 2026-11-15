mod support;

use std::collections::HashSet;
use std::str::FromStr;

use dictation_types::{
    ErrorKind, ModelDescriptor, ModelFile, ModelId, ModelInstallState, ModelKind, PlatformTarget,
    ProviderId, is_valid_sha256_hex,
};
use support::lang;

fn checksum(seed: char) -> String {
    std::iter::repeat_n(seed, 64).collect()
}

fn file(path: &str, size_bytes: u64) -> ModelFile {
    ModelFile {
        path: path.to_owned(),
        size_bytes,
        sha256: checksum('a'),
        download_urls: vec![format!("https://example.invalid/models/{path}")],
        unpack: None,
    }
}

fn model(files: Vec<ModelFile>) -> ModelDescriptor {
    ModelDescriptor {
        id: ModelId::new("model-a"),
        name: "Model A".to_owned(),
        kind: ModelKind::Asr,
        engine: ProviderId::new("test-engine"),
        group: None,
        files,
        provided_by_system: false,
        named_paths: Vec::new(),
        engine_settings: Vec::new(),
        supported_platforms: vec![PlatformTarget::LinuxX64, PlatformTarget::MacosArm64],
        supported_languages: vec![lang("pt"), lang("it"), lang("bn"), lang("ko")],
    }
}

#[test]
fn a_model_may_consist_of_several_files_each_with_its_own_details() {
    let mut encoder = file("encoder.bin", 700);
    encoder.sha256 = checksum('1');
    encoder.download_urls = vec![
        "https://mirror-one.invalid/encoder.bin".to_owned(),
        "https://mirror-two.invalid/encoder.bin".to_owned(),
    ];
    let mut decoder = file("decoder.bin", 250);
    decoder.sha256 = checksum('2');
    let mut tokens = file("tokens/vocab.txt", 50);
    tokens.sha256 = checksum('3');

    let entry = model(vec![encoder.clone(), decoder.clone(), tokens.clone()]);
    assert!(entry.validate().is_ok());
    assert_eq!(entry.files, [encoder, decoder, tokens]);
    assert_eq!(entry.files[0].download_urls.len(), 2);
    assert_ne!(entry.files[0].sha256, entry.files[1].sha256);
}

#[test]
fn the_total_size_is_the_sum_of_the_files() {
    assert_eq!(
        model(vec![file("a", 700), file("b", 250), file("c", 50)]).size_bytes(),
        1_000
    );
    assert_eq!(model(vec![file("only", 123)]).size_bytes(), 123);
    assert_eq!(model(Vec::new()).size_bytes(), 0);
}

#[test]
fn the_total_size_does_not_overflow() {
    let entry = model(vec![file("a", u64::MAX), file("b", 10)]);
    assert_eq!(entry.size_bytes(), u64::MAX);
}

#[test]
fn duplicate_file_paths_are_rejected() {
    let entry = model(vec![
        file("weights.bin", 10),
        file("config.json", 5),
        file("weights.bin", 20),
    ]);
    assert_eq!(entry.validate().unwrap_err().kind, ErrorKind::Validation);
}

#[test]
fn a_file_of_zero_size_is_rejected() {
    assert!(file("weights.bin", 0).validate().is_err());
    assert!(
        model(vec![file("ok.bin", 5), file("empty.bin", 0)])
            .validate()
            .is_err()
    );
}

#[test]
fn a_malformed_checksum_is_rejected() {
    let malformed = [
        String::new(),
        "abc123".to_owned(),
        checksum('a')[..63].to_owned(),
        format!("{}a", checksum('a')),
        checksum('g'),
        format!("{}z", &checksum('a')[..63]),
        format!("{} ", &checksum('a')[..63]),
        format!("0x{}", &checksum('a')[..62]),
        "é".repeat(32),
    ];
    for sha256 in malformed {
        assert!(!is_valid_sha256_hex(&sha256), "{sha256:?}");
        let mut bad = file("weights.bin", 10);
        bad.sha256 = sha256.clone();
        assert!(bad.validate().is_err(), "{sha256:?}");
        assert!(model(vec![bad]).validate().is_err(), "{sha256:?}");
    }
}

#[test]
fn a_well_formed_checksum_is_accepted() {
    for sha256 in [
        checksum('0'),
        checksum('f'),
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_owned(),
    ] {
        assert!(is_valid_sha256_hex(&sha256), "{sha256}");
    }
}

#[test]
fn a_file_without_sources_is_rejected() {
    let mut no_sources = file("weights.bin", 10);
    no_sources.download_urls.clear();
    assert!(no_sources.validate().is_err());
    assert!(model(vec![no_sources]).validate().is_err());
}

#[test]
fn a_file_with_a_blank_source_is_rejected() {
    for blank in ["", "   ", "\t"] {
        let mut bad = file("weights.bin", 10);
        bad.download_urls.push(blank.to_owned());
        assert!(bad.validate().is_err(), "{blank:?}");

        let mut only_blank = file("weights.bin", 10);
        only_blank.download_urls = vec![blank.to_owned()];
        assert!(only_blank.validate().is_err(), "{blank:?}");
    }
}

#[test]
fn a_file_path_is_checked_as_a_path_inside_the_model_folder() {
    assert!(file("../outside.bin", 10).validate().is_err());
}

#[test]
fn a_model_without_a_platform_is_rejected() {
    let mut entry = model(vec![file("weights.bin", 10)]);
    entry.supported_platforms.clear();
    assert!(entry.validate().is_err());
}

#[test]
fn a_model_with_a_blank_id_or_name_is_rejected() {
    for blank in ["", "   "] {
        let mut no_id = model(vec![file("weights.bin", 10)]);
        no_id.id = ModelId::new(blank);
        assert!(no_id.validate().is_err(), "id {blank:?}");

        let mut no_name = model(vec![file("weights.bin", 10)]);
        no_name.name = blank.to_owned();
        assert!(no_name.validate().is_err(), "name {blank:?}");
    }
}

#[test]
fn a_model_provided_by_the_operating_system_needs_no_files() {
    let mut entry = model(Vec::new());
    entry.provided_by_system = true;
    assert!(entry.validate().is_ok());
    assert_eq!(entry.size_bytes(), 0);
}

#[test]
fn a_downloadable_model_without_files_is_rejected() {
    assert!(model(Vec::new()).validate().is_err());
}

#[test]
fn a_model_for_any_languages_and_platforms_is_valid() {
    let mut entry = model(vec![file("weights.bin", 10)]);
    entry.supported_platforms = PlatformTarget::ALL.to_vec();
    entry.supported_languages = ["sw", "am", "yue", "haw", "zh-Hant-HK", "pt-BR"]
        .into_iter()
        .map(lang)
        .collect();
    assert!(entry.validate().is_ok());
}

#[test]
fn only_an_installed_model_can_be_loaded() {
    let loaded = ModelInstallState::Installed {
        local_path: "models/model-a".to_owned(),
        loaded_in_memory: true,
    };
    let on_disk = ModelInstallState::Installed {
        local_path: "models/model-a".to_owned(),
        loaded_in_memory: false,
    };
    assert!(loaded.is_installed() && loaded.is_loaded());
    assert!(on_disk.is_installed() && !on_disk.is_loaded());

    for state in [
        ModelInstallState::NotInstalled,
        ModelInstallState::Downloading {
            downloaded_bytes: 5,
            total_bytes: 10,
        },
        ModelInstallState::Corrupted {
            reason: "checksum".to_owned(),
        },
    ] {
        assert!(!state.is_installed(), "{state:?}");
        assert!(!state.is_loaded(), "{state:?}");
    }
    assert_eq!(
        ModelInstallState::default(),
        ModelInstallState::NotInstalled
    );
}

#[test]
fn every_platform_round_trips_through_its_text_form() {
    for target in PlatformTarget::ALL {
        assert_eq!(PlatformTarget::from_str(target.as_str()).unwrap(), target);
        assert_eq!(
            target.to_string().parse::<PlatformTarget>().unwrap(),
            target
        );
        assert_eq!(target.to_string(), target.as_str());
    }
}

#[test]
fn platform_names_are_read_case_insensitively() {
    for target in PlatformTarget::ALL {
        let upper = target.as_str().to_uppercase();
        assert_eq!(upper.parse::<PlatformTarget>().unwrap(), target, "{upper}");
    }
    assert_eq!(
        "Linux-X64".parse::<PlatformTarget>().unwrap(),
        PlatformTarget::LinuxX64
    );
    assert_eq!(
        "MacOS-Arm64".parse::<PlatformTarget>().unwrap(),
        PlatformTarget::MacosArm64
    );
}

#[test]
fn platform_names_are_all_different() {
    let names: HashSet<&str> = PlatformTarget::ALL
        .iter()
        .map(PlatformTarget::as_str)
        .collect();
    assert_eq!(names.len(), PlatformTarget::ALL.len());
    let targets: HashSet<PlatformTarget> = PlatformTarget::ALL.into_iter().collect();
    assert_eq!(targets.len(), PlatformTarget::ALL.len());
}

#[test]
fn unknown_platform_names_are_rejected() {
    for name in [
        "",
        "linux",
        "arm64",
        "solaris-sparc",
        "linux-x64-gnu",
        "linux_x64",
        "windows x64",
    ] {
        let err = name.parse::<PlatformTarget>().unwrap_err();
        assert_eq!(err.kind, ErrorKind::Validation, "{name:?}");
    }
}
