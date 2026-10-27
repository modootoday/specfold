//! Per-file guard results from earlier whole-tree runs, in the user's cache directory, keyed by
//! each file's size and modification time and by everything that decides a result (the guard's
//! definition, its denied terms, the specfold version). Published trees rarely change, so a warm
//! run reads only the files that did.

use crate::check::finding::Finding;
use crate::config::{GuardDef, paths};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const FORMAT: &str = concat!("specfold-guard-2/", env!("CARGO_PKG_VERSION"));

#[derive(Serialize, Deserialize)]
struct Entry {
    mtime_ns: u128,
    size: u64,
    /// Content hash, so a fresh checkout of the same bytes (a gate tree) still hits.
    sha256: String,
    findings: Vec<Hit>,
}

/// A finding without its path, which is the entry's key.
#[derive(Serialize, Deserialize, Clone)]
struct Hit {
    line: usize,
    message: String,
    error: bool,
}

#[derive(Serialize, Deserialize, Default)]
struct Stored {
    format: String,
    key: String,
    entries: HashMap<String, Entry>,
}

pub struct GuardCache {
    file: Option<PathBuf>,
    stored: Stored,
    seen: HashMap<String, Entry>,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn stamp_of(path: &Path) -> Option<(u128, u64)> {
    let m = std::fs::metadata(path).ok()?;
    let mtime = m.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_nanos();
    Some((mtime, m.len()))
}

fn sha_of(path: &Path) -> Option<String> {
    std::fs::read(path).ok().map(|b| hex(&Sha256::digest(&b)))
}

impl GuardCache {
    /// `terms` is the compiled denied-term pattern, part of the key but never stored.
    pub fn open(root: &Path, g: &GuardDef, terms: &str) -> GuardCache {
        let dir = if cfg!(test) { None } else { paths::cache_dir() };
        let place = Sha256::digest(root.to_string_lossy().as_bytes());
        let safe: String = g.name.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
        let file = dir.map(|d| d.join(format!("guard-{}-{safe}.json", hex(&place[..8]))));
        let def = serde_json::to_string(g).unwrap_or_default();
        let key = hex(&Sha256::digest(format!("{def}\n{terms}").as_bytes()));
        let stored = file
            .as_ref()
            .and_then(|f| std::fs::read(f).ok())
            .and_then(|b| serde_json::from_slice::<Stored>(&b).ok())
            .filter(|s| s.format == FORMAT && s.key == key)
            .unwrap_or(Stored { format: FORMAT.into(), key, entries: HashMap::new() });
        GuardCache { file, stored, seen: HashMap::new() }
    }

    /// The findings recorded for `rel` when its bytes have not changed: the same size and
    /// modification time, or else the same content hash. A hit is carried into what is saved.
    pub fn lookup(&mut self, root: &Path, rel: &str) -> Option<Vec<Finding>> {
        let path = root.join(rel);
        let (mtime, size) = stamp_of(&path)?;
        let e = self.stored.entries.get(rel).filter(|e| e.size == size)?;
        if e.mtime_ns != mtime && sha_of(&path)? != e.sha256 {
            return None;
        }
        let mut e = self.stored.entries.remove(rel)?;
        e.mtime_ns = mtime;
        let found = e.findings.iter().map(|h| h.to_finding(rel)).collect();
        self.seen.insert(rel.to_string(), e);
        Some(found)
    }

    pub fn put(&mut self, root: &Path, rel: &str, findings: &[Finding]) {
        let path = root.join(rel);
        let (Some((mtime_ns, size)), Some(sha256)) = (stamp_of(&path), sha_of(&path)) else { return };
        let findings = findings.iter().filter(|f| f.path == rel).map(Hit::from).collect();
        self.seen.insert(rel.to_string(), Entry { mtime_ns, size, sha256, findings });
    }

    /// Saves the entries this run saw, dropping files that are gone. Failures are ignored: the
    /// cache is an optimisation.
    pub fn save(self) {
        let Some(file) = self.file else { return };
        let Some(dir) = file.parent() else { return };
        if std::fs::create_dir_all(dir).is_err() {
            return;
        }
        let stored = Stored { format: self.stored.format, key: self.stored.key, entries: self.seen };
        let tmp = file.with_extension(format!("tmp{}", std::process::id()));
        if let Ok(bytes) = serde_json::to_vec(&stored)
            && std::fs::write(&tmp, bytes).is_ok()
        {
            let _ = std::fs::rename(&tmp, &file);
        }
    }
}

impl Hit {
    fn to_finding(&self, path: &str) -> Finding {
        let f = if self.error { Finding::error } else { Finding::warn };
        f("U1", path, self.line, self.message.clone())
    }
}

impl From<&Finding> for Hit {
    fn from(f: &Finding) -> Self {
        Hit { line: f.line, message: f.message.clone(), error: f.severity == crate::check::finding::Severity::Error }
    }
}
