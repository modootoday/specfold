//! Attachment hashes remembered between runs, in the user's cache directory, keyed by each
//! file's size and modification time. Checking that recorded hashes still match then reads only
//! the files that changed.

use super::hash;
use crate::config::paths;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const FORMAT: &str = "specfold-hashes-1";

#[derive(Serialize, Deserialize, Clone)]
struct Entry {
    mtime_ns: u128,
    size: u64,
    sha256: String,
}

#[derive(Serialize, Deserialize, Default)]
struct Stored {
    format: String,
    entries: HashMap<String, Entry>,
}

pub struct HashCache {
    file: Option<PathBuf>,
    stored: Stored,
    dirty: bool,
}

impl HashCache {
    pub fn open(root: &Path) -> HashCache {
        let dir = if cfg!(test) { None } else { paths::cache_dir() };
        let key = Sha256::digest(root.to_string_lossy().as_bytes());
        let name: String = key[..8].iter().map(|b| format!("{b:02x}")).collect();
        let file = dir.map(|d| d.join(format!("hashes-{name}.json")));
        let stored = file
            .as_ref()
            .and_then(|f| std::fs::read(f).ok())
            .and_then(|b| serde_json::from_slice::<Stored>(&b).ok())
            .filter(|s| s.format == FORMAT)
            .unwrap_or(Stored { format: FORMAT.into(), entries: HashMap::new() });
        HashCache { file, stored, dirty: false }
    }

    /// SHA-256 (hex) and size of `path`, read only when its size or modification time changed.
    pub fn file(&mut self, path: &Path) -> std::io::Result<(String, u64)> {
        let meta = std::fs::metadata(path)?;
        let mtime_ns = meta.modified()?.duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let key = path.to_string_lossy().into_owned();
        if let Some(e) = self.stored.entries.get(&key).filter(|e| e.mtime_ns == mtime_ns && e.size == meta.len()) {
            return Ok((e.sha256.clone(), e.size));
        }
        let (sha256, size) = hash::file(path)?;
        self.stored.entries.insert(key, Entry { mtime_ns, size, sha256: sha256.clone() });
        self.dirty = true;
        Ok((sha256, size))
    }

    /// Writes the cache when it changed. Failures are ignored: the cache is an optimisation.
    pub fn save(&self) {
        let (Some(file), true) = (&self.file, self.dirty) else { return };
        let Some(dir) = file.parent() else { return };
        if std::fs::create_dir_all(dir).is_err() {
            return;
        }
        let tmp = file.with_extension(format!("tmp{}", std::process::id()));
        if let Ok(bytes) = serde_json::to_vec(&self.stored)
            && std::fs::write(&tmp, bytes).is_ok()
        {
            let _ = std::fs::rename(&tmp, file);
        }
    }
}
