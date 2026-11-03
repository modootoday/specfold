//! Path resolution for the paths commands are given, kept in one place for every command.

/// An absolute path for a file that may not exist yet: the nearest existing ancestor is resolved.
pub fn absolute(p: &std::path::Path) -> std::path::PathBuf {
    let p = if p.is_absolute() { p.to_path_buf() } else { std::env::current_dir().unwrap_or_default().join(p) };
    specfold_core::paths::real_with_missing_tail(&p).unwrap_or(p)
}
