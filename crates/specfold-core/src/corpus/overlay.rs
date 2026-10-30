//! Unsaved text for open files: an editor checks its buffers before they are written to disk.
//! A buffer for a file that does not exist yet joins the walk when it lies in a spec directory.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

static OVERLAY: Mutex<Option<HashMap<PathBuf, String>>> = Mutex::new(None);

fn map() -> MutexGuard<'static, Option<HashMap<PathBuf, String>>> {
    OVERLAY.lock().unwrap_or_else(|e| e.into_inner())
}

/// Reads of `path` (absolute) return `text` until it is cleared.
pub fn set(path: PathBuf, text: String) {
    map().get_or_insert_with(HashMap::new).insert(path, text);
}

/// Back to the file on disk.
pub fn clear(path: &Path) {
    if let Some(m) = map().as_mut() {
        m.remove(path);
    }
}

/// The file's text, from the overlay when it holds this path, otherwise from disk.
pub fn read(path: &Path) -> std::io::Result<String> {
    if let Some(t) = map().as_ref().and_then(|m| m.get(path)) {
        return Ok(t.clone());
    }
    let bytes = std::fs::read(path)?;
    String::from_utf8(bytes).map_err(|e| {
        let at = e.utf8_error().valid_up_to();
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("{}: {}", path.display(), utf8_message(&e.as_bytes()[..at])),
        )
    })
}

/// Where a file stops being UTF-8: `not valid UTF-8 at line N (byte B)`, from the valid prefix.
pub fn utf8_message(valid: &[u8]) -> String {
    let line = valid.iter().filter(|b| **b == b'\n').count() + 1;
    format!("{UTF8} at line {line} (byte {})", valid.len())
}

/// The start of every message about a file that is not UTF-8.
pub const UTF8: &str = "not valid UTF-8";

/// The line `utf8_message` named, for a finding.
pub fn utf8_line(msg: &str) -> Option<usize> {
    msg.split_once(" at line ")?.1.split(' ').next()?.parse().ok()
}

/// Whether the overlay holds this path, so a cache keyed on disk stamps rescans it.
pub fn covers(path: &Path) -> bool {
    map().as_ref().is_some_and(|m| m.contains_key(path))
}

/// Buffers under `dir` with no file on disk yet.
pub fn unsaved_under(dir: &Path) -> Vec<PathBuf> {
    map()
        .as_ref()
        .map(|m| m.keys().filter(|p| p.starts_with(dir) && !p.exists()).cloned().collect())
        .unwrap_or_default()
}

/// A hash of every buffer held, so a cache of whole-corpus results knows when buffers changed.
pub fn stamp() -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    if let Some(m) = map().as_ref() {
        let mut v: Vec<_> = m.iter().collect();
        v.sort();
        v.hash(&mut h);
    }
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_read_clear() {
        let p = PathBuf::from("/nonexistent/overlay-test/a.md");
        set(p.clone(), "x".into());
        assert_eq!(read(&p).unwrap(), "x");
        assert!(covers(&p));
        assert_eq!(unsaved_under(Path::new("/nonexistent/overlay-test")), vec![p.clone()]);
        clear(&p);
        assert!(read(&p).is_err());
    }
}
