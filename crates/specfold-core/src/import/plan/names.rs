//! Names an import reads from source files: the file name, its stem, and a title's number.

pub(super) fn name_of(p: &str) -> &str {
    p.rsplit('/').next().unwrap_or(p)
}

/// A file's name without `.md`, or a directory's name.
pub(super) fn stem_of(p: &str) -> &str {
    name_of(p).trim_end_matches(".md")
}

/// `2. Use X` → `Use X`; a title that is only a number stays.
pub(super) fn strip_number(title: &str) -> String {
    let digits = title.chars().take_while(char::is_ascii_digit).count();
    match title[digits..].strip_prefix('.') {
        Some(rest) if digits > 0 && !rest.trim().is_empty() => rest.trim().to_string(),
        _ => title.to_string(),
    }
}
