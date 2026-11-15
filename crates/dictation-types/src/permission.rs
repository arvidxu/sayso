use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PermissionKind {
    Microphone,
    Accessibility,
    InputMonitoring,
}

impl fmt::Display for PermissionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Microphone => write!(f, "microphone"),
            Self::Accessibility => write!(f, "accessibility"),
            Self::InputMonitoring => write!(f, "input_monitoring"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PermissionStatus {
    Granted,
    Denied,
    #[default]
    NotDetermined,
    Restricted,
}

impl PermissionStatus {
    pub fn is_granted(&self) -> bool {
        matches!(self, Self::Granted)
    }
}

/// Which permissions are required is up to the platform; an empty report means none.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PermissionReport {
    pub permissions: Vec<(PermissionKind, PermissionStatus)>,
}

impl PermissionReport {
    pub fn new(permissions: Vec<(PermissionKind, PermissionStatus)>) -> Self {
        Self { permissions }
    }

    /// `NotDetermined` when the report does not mention `kind`.
    pub fn status_of(&self, kind: PermissionKind) -> PermissionStatus {
        self.permissions
            .iter()
            .find_map(|(k, status)| (*k == kind).then_some(*status))
            .unwrap_or(PermissionStatus::NotDetermined)
    }

    /// `true` for an empty report.
    pub fn all_granted(&self) -> bool {
        self.permissions.iter().all(|(_, s)| s.is_granted())
    }

    pub fn missing(&self) -> Vec<PermissionKind> {
        self.permissions
            .iter()
            .filter_map(|(kind, status)| (!status.is_granted()).then_some(*kind))
            .collect()
    }
}
