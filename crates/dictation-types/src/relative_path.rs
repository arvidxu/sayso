use crate::{AppError, AppResult};

/// Reserved for devices on one supported system, whatever the extension.
const RESERVED_NAMES: &[&str] = &["con", "prn", "aux", "nul"];

/// Reserved for devices on that system when followed by one digit other than zero.
const RESERVED_NUMBERED: &[&str] = &["com", "lpt"];

/// Not allowed in file names on that system.
const FORBIDDEN_CHARS: &[char] = &['<', '>', ':', '"', '|', '?', '*', '\\'];

fn is_reserved(segment: &str) -> bool {
    let stem = segment
        .split('.')
        .next()
        .unwrap_or(segment)
        .to_ascii_lowercase();
    RESERVED_NAMES.contains(&stem.as_str())
        || RESERVED_NUMBERED.iter().any(|prefix| {
            stem.strip_prefix(prefix).is_some_and(|rest| {
                rest.len() == 1 && rest != "0" && rest.bytes().all(|b| b.is_ascii_digit())
            })
        })
}

/// Relative, `/`-separated, staying inside its folder, and storable on every supported
/// system.
pub fn check_relative_path(path: &str, what: &str) -> AppResult<()> {
    let refuse = |why: &str| {
        Err(AppError::validation(format!(
            "{what} '{}' {why}",
            path.escape_debug()
        )))
    };
    if path.trim().is_empty() {
        return refuse("must not be empty");
    }
    if path.starts_with('/') || path.starts_with('~') {
        return refuse("must be relative to the model's folder");
    }
    if path != path.trim() || path.chars().any(char::is_control) {
        return refuse("is not a plain relative path");
    }
    if let Some(ch) = path.chars().find(|ch| FORBIDDEN_CHARS.contains(ch)) {
        return refuse(&format!("contains '{ch}', which not every system allows"));
    }
    for segment in path.split('/') {
        if segment.is_empty() || segment == "." {
            return refuse("has an empty part");
        }
        if segment == ".." {
            return refuse("must stay inside the model's folder");
        }
        if segment.ends_with('.') || segment.ends_with(' ') {
            return refuse("has a name ending in a dot or a space");
        }
        if is_reserved(segment) {
            return refuse("uses a name Windows reserves for devices");
        }
    }
    Ok(())
}
