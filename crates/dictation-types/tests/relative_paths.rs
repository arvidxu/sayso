use dictation_types::check_relative_path;

#[test]
fn plain_relative_paths_are_accepted() {
    for path in [
        "model.onnx",
        "encoder/weights.bin",
        "a/b/c/tokens.txt",
        "name with spaces/file-1_v2.txt",
        "Модель/вес.bin",
        "模型/权重.bin",
        ".hidden",
        "console.txt",
        "com10",
        "com0",
    ] {
        assert!(check_relative_path(path, "path").is_ok(), "{path}");
    }
}

#[test]
fn paths_that_leave_the_folder_or_are_absolute_are_refused() {
    for path in ["/etc/passwd", "~/model", "../model", "a/../../b", "a/.."] {
        assert!(check_relative_path(path, "path").is_err(), "{path}");
    }
}

#[test]
fn malformed_paths_are_refused() {
    for path in [
        "", " ", " a", "a ", "a//b", "a/./b", "./a", "a/", "a\tb", "a\nb",
    ] {
        assert!(check_relative_path(path, "path").is_err(), "{path:?}");
    }
}

#[test]
fn backslashes_and_characters_windows_cannot_store_are_refused() {
    for path in [
        "a\\b", "C:model", "a<b", "a>b", "a:b", "a\"b", "a|b", "a?b", "a*b",
    ] {
        assert!(check_relative_path(path, "path").is_err(), "{path}");
    }
}

#[test]
fn names_windows_reserves_or_cannot_end_with_are_refused() {
    for path in [
        "CON", "con.txt", "a/nul", "Aux.bin", "prn", "COM1", "com9.log", "LPT3", "a/b.", "a/b ",
        "dots...",
    ] {
        assert!(check_relative_path(path, "path").is_err(), "{path}");
    }
}

#[test]
fn the_error_names_what_was_checked() {
    let err = check_relative_path("../x", "model file path").unwrap_err();
    assert!(err.message.contains("model file path"), "{err:?}");
}
