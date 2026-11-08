use std::io;
use std::path::Path;

/// The OS reason for an I/O failure without the trailing "(os error N)" code.
pub fn io_reason(error: &io::Error) -> String {
    let text = error.to_string();
    let reason = match text.rfind(" (os error ") {
        Some(at) if text.ends_with(')') => &text[..at],
        _ => &text,
    };
    reason.trim_end_matches('.').to_owned()
}

/// "Could not {action} "{path}": {reason}." for messages shown to the user.
pub fn io_failure(action: &str, path: &Path, error: &io::Error) -> String {
    format!("Could not {action} \"{}\": {}.", path.display(), io_reason(error))
}

/// Starts with a capital letter and ends with a full stop, for core errors written in lowercase.
pub fn sentence(text: impl AsRef<str>) -> String {
    let text = text.as_ref().trim();
    let mut chars = text.chars();
    let mut out: String = match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => return String::new(),
    };
    if !out.ends_with(['.', '!', '?']) {
        out.push('.');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_codes_are_dropped() {
        let error = io::Error::from_raw_os_error(2);
        assert!(!io_reason(&error).contains("os error"), "{}", io_reason(&error));
    }

    #[test]
    fn sentences_are_capitalized_and_closed() {
        assert_eq!(sentence("the pool is empty"), "The pool is empty.");
        assert_eq!(sentence("Done."), "Done.");
        assert_eq!(sentence(""), "");
    }
}
