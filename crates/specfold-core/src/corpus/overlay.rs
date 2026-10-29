//! Unsaved text for one file: an editor checks its buffer before it is written to disk.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static OVERLAY: OnceLock<(PathBuf, String)> = OnceLock::new();

/// Reads of `path` (absolute) return `text` for the rest of the process. Set once, before loading.
pub fn set(path: PathBuf, text: String) {
    let _ = OVERLAY.set((path, text));
}

/// The file's text, from the overlay when it names this path, otherwise from disk.
pub fn read(path: &Path) -> std::io::Result<String> {
    match OVERLAY.get() {
        Some((p, t)) if p == path => Ok(t.clone()),
        _ => std::fs::read_to_string(path),
    }
}

/// Whether the overlay names this path, so a cache keyed on disk stamps rescans it.
pub fn covers(path: &Path) -> bool {
    OVERLAY.get().is_some_and(|(p, _)| p == path)
}
