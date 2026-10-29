//! The scan cache: one file per repository in the user's cache directory, keyed by each file's
//! size and modification time. Only files that changed are read and parsed again. The cache is
//! never stored in the repository and losing it only costs one slower run.

use super::scan::{FileScan, scan};
use crate::config::{Config, paths};
use rayon::prelude::*;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Bumped whenever the scan output changes shape.
const FORMAT: &str = concat!("specfold-scan-1/", env!("CARGO_PKG_VERSION"));

#[derive(Serialize, Deserialize)]
struct Entry {
    mtime_ns: u128,
    size: u64,
    scan: FileScan,
}

#[derive(Serialize, Deserialize, Default)]
struct Stored {
    format: String,
    spec_dir: String,
    entries: HashMap<String, Entry>,
}

pub struct ScanCache {
    file: Option<PathBuf>,
    stored: Stored,
    dirty: bool,
}

fn stamp_of(path: &Path) -> Option<(u128, u64)> {
    let m = std::fs::metadata(path).ok()?;
    let mtime = m.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_nanos();
    Some((mtime, m.len()))
}

impl ScanCache {
    pub fn open(cfg: &Config) -> Self {
        // Unit tests load throwaway repositories; they must not leave files in the user's cache.
        let dir = if cfg!(test) { None } else { paths::cache_dir() };
        let file = dir.map(|d| {
            let key = Sha256::digest(cfg.root.to_string_lossy().as_bytes());
            d.join(format!("scan-{}.json", super::scan::hex(&key[..8])))
        });
        let stored = file
            .as_ref()
            .and_then(|f| std::fs::read(f).ok())
            .and_then(|b| serde_json::from_slice::<Stored>(&b).ok())
            .filter(|s| s.format == FORMAT && s.spec_dir == cfg.spec_dir())
            .unwrap_or_else(|| Stored {
                format: FORMAT.into(),
                spec_dir: cfg.spec_dir().into(),
                entries: HashMap::new(),
            });
        ScanCache { file, stored, dirty: false }
    }

    /// Scans every path (repository-relative), reusing cached results for unchanged files. The
    /// cache keeps no copy unless it must be saved, so a warm run moves results instead of cloning.
    pub fn scan_all(&mut self, root: &Path, rels: &[String], cite_re: &Regex) -> HashMap<String, FileScan> {
        let stamps: Vec<Option<(u128, u64)>> = rels.par_iter().map(|rel| stamp_of(&root.join(rel))).collect();
        let fresh: Vec<bool> = rels
            .iter()
            .zip(&stamps)
            .map(|(rel, st)| match (st, self.stored.entries.get(rel)) {
                (Some((m, s)), Some(e)) => e.mtime_ns != *m || e.size != *s || super::overlay::covers(&root.join(rel)),
                _ => true,
            })
            .collect();
        let rescanned: Vec<Option<FileScan>> = rels
            .par_iter()
            .zip(&fresh)
            .map(|(rel, f)| f.then(|| scan(&super::overlay::read(&root.join(rel)).unwrap_or_default(), cite_re)))
            .collect();
        let stale = self.stored.entries.len() != rels.len() || fresh.iter().any(|f| *f);
        self.dirty = stale;
        let mut old = std::mem::take(&mut self.stored.entries);
        let mut out = HashMap::with_capacity(rels.len());
        for ((rel, st), new) in rels.iter().zip(stamps).zip(rescanned) {
            let scan = match new {
                Some(s) => s,
                None => match old.remove(rel) {
                    Some(e) => e.scan,
                    None => continue,
                },
            };
            if stale
                && !super::overlay::covers(&root.join(rel))
                && let Some((mtime_ns, size)) = st
            {
                self.stored.entries.insert(rel.clone(), Entry { mtime_ns, size, scan: scan.clone() });
            }
            out.insert(rel.clone(), scan);
        }
        out
    }

    /// Writes the cache atomically when it changed. Failures are ignored: the cache is an
    /// optimisation.
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
