use dictation_types::{ErrorKind, SecretString};

const SECRET: &str = "tok-4f9a-VERY-secret-ключ-秘密";

fn assert_hidden(rendered: &str) {
    assert!(!rendered.contains(SECRET), "secret leaked: {rendered}");
    for fragment in ["4f9a", "VERY-secret", "ключ", "秘密"] {
        assert!(
            !rendered.contains(fragment),
            "part of the secret leaked: {rendered}"
        );
    }
}

#[test]
fn a_secret_never_appears_in_display_output() {
    let secret = SecretString::new(SECRET);
    assert_hidden(&secret.to_string());
    assert_hidden(&format!("{secret}"));
    assert_hidden(&format!("{secret:>80}"));
}

#[test]
fn a_secret_never_appears_in_debug_output() {
    let secret = SecretString::new(SECRET);
    assert_hidden(&format!("{secret:?}"));
    assert_hidden(&format!("{secret:#?}"));
}

#[test]
fn a_secret_is_only_revealed_on_request() {
    let secret = SecretString::new(SECRET);
    assert_eq!(secret.expose_secret(), SECRET);
    assert!(!secret.is_empty());
    assert_eq!(secret, SecretString::from(SECRET));
}

#[test]
fn a_validated_secret_cannot_be_blank() {
    for blank in ["", " ", "\t\n"] {
        assert_eq!(
            SecretString::try_new(blank).unwrap_err().kind,
            ErrorKind::Validation
        );
    }
    assert!(SecretString::try_new(SECRET).is_ok());
    assert!(SecretString::new("").is_empty());
}
