use crate::{InjectionWarning, Mode, PromptVariable};

/// The kind of application that has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppScene {
    #[default]
    General,
    Chat,
    CodeEditor,
    Terminal,
    Document,
    Email,
    Browser,
    PasswordField,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ActiveAppInfo {
    pub app_name: String,
    pub bundle_id_or_exe: Option<String>,
    pub window_title: Option<String>,
    pub process_id: Option<u32>,
    pub is_elevated_admin: bool,
    pub is_secure_input_active: bool,
}

impl ActiveAppInfo {
    pub fn new(app_name: impl Into<String>) -> Self {
        Self {
            app_name: app_name.into(),
            ..Self::default()
        }
    }

    pub fn with_identifier(mut self, bundle_id_or_exe: impl Into<String>) -> Self {
        self.bundle_id_or_exe = Some(bundle_id_or_exe.into());
        self
    }

    pub fn injection_warning(&self) -> Option<InjectionWarning> {
        if self.is_secure_input_active {
            Some(InjectionWarning::SecureInputActive)
        } else if self.is_elevated_admin {
            Some(InjectionWarning::ElevatedAdminWindow)
        } else {
            None
        }
    }
}

/// The selection and the clipboard are read only when a mode needs them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ContextQuery {
    pub include_active_app: bool,
    pub include_selected_text: bool,
    pub include_clipboard: bool,
}

impl ContextQuery {
    /// Only the foreground application.
    pub fn minimal() -> Self {
        Self {
            include_active_app: true,
            include_selected_text: false,
            include_clipboard: false,
        }
    }

    pub fn all() -> Self {
        Self {
            include_active_app: true,
            include_selected_text: true,
            include_clipboard: true,
        }
    }

    /// The selection and the clipboard only if `mode`'s prompt asks for them.
    pub fn for_mode(mode: &Mode) -> Self {
        Self {
            include_active_app: true,
            include_selected_text: mode.uses(PromptVariable::SelectedText),
            include_clipboard: mode.uses(PromptVariable::ClipboardText),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AppContext {
    pub active_app: Option<ActiveAppInfo>,
    pub scene: AppScene,
    pub selected_text: Option<String>,
    pub clipboard_text: Option<String>,
}

impl AppContext {
    pub fn new(scene: AppScene) -> Self {
        Self {
            scene,
            ..Self::default()
        }
    }

    pub fn with_active_app(mut self, active_app: ActiveAppInfo) -> Self {
        self.active_app = Some(active_app);
        self
    }

    pub fn with_selected_text(mut self, selected_text: impl Into<String>) -> Self {
        self.selected_text = Some(selected_text.into());
        self
    }

    pub fn with_clipboard_text(mut self, clipboard_text: impl Into<String>) -> Self {
        self.clipboard_text = Some(clipboard_text.into());
        self
    }

    pub fn injection_warning(&self) -> Option<InjectionWarning> {
        if self.scene == AppScene::PasswordField {
            return Some(InjectionWarning::SecureInputActive);
        }
        self.active_app
            .as_ref()
            .and_then(ActiveAppInfo::injection_warning)
    }
}
