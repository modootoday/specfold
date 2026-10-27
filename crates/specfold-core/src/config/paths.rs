//! Locations on the user's machine: home, cache, and `~` expansion.

use std::path::{Path, PathBuf};

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from)
}

/// The per-user cache directory: `$SPECFOLD_CACHE`, `$XDG_CACHE_HOME`, `%LOCALAPPDATA%`,
/// `~/Library/Caches` or `~/.cache`.
pub fn cache_dir() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("SPECFOLD_CACHE") {
        return Some(PathBuf::from(p));
    }
    if let Some(p) = std::env::var_os("XDG_CACHE_HOME") {
        return Some(PathBuf::from(p).join("specfold"));
    }
    if cfg!(windows) {
        return std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("specfold"));
    }
    let home = home_dir()?;
    if cfg!(target_os = "macos") {
        return Some(home.join("Library/Caches/specfold"));
    }
    Some(home.join(".cache/specfold"))
}

/// Expands `~/` and resolves a relative path against `base`.
pub fn expand(base: &Path, p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/")
        && let Some(home) = home_dir()
    {
        return home.join(rest);
    }
    let path = PathBuf::from(p);
    if path.is_absolute() { path } else { base.join(path) }
}
