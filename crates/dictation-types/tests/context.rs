use dictation_types::{ActiveAppInfo, AppContext, AppScene, InjectionWarning};

#[test]
fn secure_input_is_warned_about_before_an_elevated_window() {
    let mut app = ActiveAppInfo::new("Terminal").with_identifier("example.terminal");
    assert_eq!(app.injection_warning(), None);
    app.is_elevated_admin = true;
    assert_eq!(
        app.injection_warning(),
        Some(InjectionWarning::ElevatedAdminWindow)
    );
    app.is_secure_input_active = true;
    assert_eq!(
        app.injection_warning(),
        Some(InjectionWarning::SecureInputActive)
    );
}

#[test]
fn a_password_field_always_warns_about_secure_input() {
    let context = AppContext::new(AppScene::PasswordField);
    assert_eq!(
        context.injection_warning(),
        Some(InjectionWarning::SecureInputActive)
    );

    let mut app = ActiveAppInfo::new("Editor");
    app.is_elevated_admin = true;
    let elevated = AppContext::new(AppScene::CodeEditor).with_active_app(app);
    assert_eq!(
        elevated.injection_warning(),
        Some(InjectionWarning::ElevatedAdminWindow)
    );

    assert_eq!(AppContext::new(AppScene::Chat).injection_warning(), None);
}

#[test]
fn a_context_holds_exactly_what_it_was_given() {
    let context = AppContext::new(AppScene::Email)
        .with_selected_text("выделенный текст")
        .with_clipboard_text("クリップボード");
    assert_eq!(context.scene, AppScene::Email);
    assert_eq!(context.selected_text.as_deref(), Some("выделенный текст"));
    assert_eq!(context.clipboard_text.as_deref(), Some("クリップボード"));
    assert_eq!(context.active_app, None);

    let empty = AppContext::new(AppScene::General);
    assert_eq!(empty.selected_text, None);
    assert_eq!(empty.clipboard_text, None);
}
