//! L6's per-file pass: the references a file holds, read in parallel and, for whole-tree runs,
//! kept in a cache keyed by each file's size and modification time (B2). A cached file is read
//! again whenever a path token it holds may now name a document, so the cache never hides a
//! finding: a hash collision only costs one extra read.

use crate::config::{Config, paths};
use crate::corpus::{overlay, trace};
use rayon::prelude::*;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Bumped whenever extraction changes.
const FORMAT: &str = concat!("specfold-cites-1/", env!("CARGO_PKG_VERSION"));

/// One `[[…]]` reference: line, the text as written, the target inside the brackets.
pub type Wiki = (usize, String, String);

/// What L6 needs from one file, independent of the corpus.
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Extract {
    pub wiki: Vec<Wiki>,
    /// Path tokens per line, after the line's `[[…]]` targets and repeats are dropped.
    pub tokens: Vec<(usize, String)>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Entry {
    mtime_ns: u128,
    size: u64,
    wiki: Vec<Wiki>,
    /// Tokens that named a document when stored, with their lines.
    known: Vec<(usize, String)>,
    /// Hashes of every distinct token, to tell whether a new document name occurs in the file.
    hashes: Vec<u64>,
}

#[derive(Serialize, Deserialize, Default)]
struct Stored {
    format: String,
    key: String,
    entries: HashMap<String, Entry>,
}

pub fn token_hash(t: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    t.hash(&mut h);
    h.finish()
}

/// The patterns that decide what a file yields.
pub struct Patterns<'a> {
    pub wiki: &'a Regex,
    pub spec_paths: &'a Regex,
    pub token: &'a Regex,
    pub history: &'a [Regex],
}

pub fn extract(text: &str, only: Option<&BTreeSet<usize>>, p: &Patterns) -> Extract {
    let mut out = Extract::default();
    for (n, line) in text.lines().enumerate() {
        let no = n + 1;
        if only.is_some_and(|set| !set.contains(&no)) || p.history.iter().any(|h| h.is_match(line)) {
            continue;
        }
        let mut seen = BTreeSet::new();
        for c in p.wiki.captures_iter(line) {
            let target = c[1].trim().to_string();
            seen.insert(target.clone());
            out.wiki.push((no, c[0].to_string(), target));
        }
        for m in p.spec_paths.find_iter(line).chain(p.token.find_iter(line)) {
            let t = m.as_str().trim_end_matches(['.', ',', ':', ';']);
            if seen.insert(t.to_string()) {
                out.tokens.push((no, t.to_string()));
            }
        }
    }
    out
}

/// Counters for `SPECFOLD_TRACE`.
#[derive(Default)]
struct Stats {
    disk: AtomicUsize,
    git: AtomicUsize,
    cached: AtomicUsize,
    bytes: AtomicU64,
    read_us: AtomicU64,
}

/// Where a file's current text comes from.
pub enum Source<'a> {
    /// Disk (through the editor overlay); cacheable.
    Disk,
    /// The index, for staged runs (`git show :path`), except paths in `in_submodules`.
    Staged(&'a BTreeSet<String>),
}

fn stamp(path: &Path) -> Option<(u128, u64)> {
    let m = std::fs::metadata(path).ok()?;
    Some((m.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_nanos(), m.len()))
}

/// A file modified within this window may change again in the same timestamp tick; it is not stored.
const RACY_NS: u128 = 2_000_000_000;

fn cache_file(cfg: &Config) -> Option<PathBuf> {
    let dir = if cfg!(test) { None } else { paths::cache_dir() }?;
    let key = Sha256::digest(cfg.root.to_string_lossy().as_bytes());
    Some(dir.join(format!("cites-{}.json", crate::corpus::scan::hex(&key[..8]))))
}

/// Extracts every target, reusing the cache for unchanged files when `use_cache` holds.
/// `known` tells whether a token names a document now.
pub fn run(
    cfg: &Config,
    targets: &[(String, Option<BTreeSet<usize>>)],
    src: Source,
    p: &Patterns,
    known: &(dyn Fn(&str) -> bool + Sync),
    names: &HashSet<u64>,
) -> Vec<Extract> {
    let start = Instant::now();
    let use_cache = matches!(src, Source::Disk) && targets.iter().all(|(_, o)| o.is_none());
    let key = format!("{}\n{}", cfg.spec_dir(), p.history.iter().map(|r| r.as_str()).collect::<Vec<_>>().join("\n"));
    let file = if use_cache { cache_file(cfg) } else { None };
    if let Some(f) = &file
        && let Some(d) = f.parent()
    {
        crate::config::cache_gc::prune(d);
        crate::config::cache_gc::touch(f);
    }
    let stored = file
        .as_ref()
        .and_then(|f| std::fs::read(f).ok())
        .and_then(|b| serde_json::from_slice::<Stored>(&b).ok())
        .filter(|s| s.format == FORMAT && s.key == key)
        .unwrap_or_default();
    let loaded = start.elapsed();
    let stats = Stats::default();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let root = &cfg.root;
    let results: Vec<(Extract, Option<Entry>, bool)> = targets
        .par_iter()
        .map(|(rel, only)| {
            let abs = root.join(rel);
            let st = if use_cache && !overlay::covers(&abs) { stamp(&abs) } else { None };
            if let (Some((m, s)), Some(e)) = (st, stored.entries.get(rel))
                && e.mtime_ns == m
                && e.size == s
                && reusable(e, known, names)
            {
                stats.cached.fetch_add(1, Ordering::Relaxed);
                let ex = Extract { wiki: e.wiki.clone(), tokens: e.known.clone() };
                return (ex, None, true);
            }
            let t0 = Instant::now();
            let text = match &src {
                Source::Staged(subs) if !subs.contains(rel) => {
                    stats.git.fetch_add(1, Ordering::Relaxed);
                    crate::workspace::vcs::show_staged(root, rel)
                }
                _ => {
                    stats.disk.fetch_add(1, Ordering::Relaxed);
                    overlay::read(&abs).ok()
                }
            };
            stats.read_us.fetch_add(t0.elapsed().as_micros() as u64, Ordering::Relaxed);
            let Some(text) = text else { return (Extract::default(), None, false) };
            stats.bytes.fetch_add(text.len() as u64, Ordering::Relaxed);
            let ex = extract(&text, only.as_ref(), p);
            let entry = st.filter(|(m, _)| now.saturating_sub(*m) > RACY_NS).map(|(mtime_ns, size)| Entry {
                mtime_ns,
                size,
                wiki: ex.wiki.clone(),
                known: ex.tokens.iter().filter(|(_, t)| known(t)).cloned().collect(),
                hashes: ex.tokens.iter().map(|(_, t)| token_hash(t)).collect::<BTreeSet<_>>().into_iter().collect(),
            });
            (ex, entry, false)
        })
        .collect();
    let read_wall = start.elapsed() - loaded;
    if let Some(f) = &file {
        let fresh = results.iter().filter(|r| !r.2).count();
        if fresh > 0 || stored.entries.len() != targets.len() {
            save(f, stored, key, targets, &results);
        }
    }
    if trace::enabled() {
        let s = &stats;
        eprintln!(
            "specfold:   L6 files     {} (disk {}, git {}, cached {}), {:.1} MB read; read {:.1} ms summed over threads, read+extract {:.1} ms wall, cache load {:.1} ms",
            targets.len(),
            s.disk.load(Ordering::Relaxed),
            s.git.load(Ordering::Relaxed),
            s.cached.load(Ordering::Relaxed),
            s.bytes.load(Ordering::Relaxed) as f64 / 1e6,
            s.read_us.load(Ordering::Relaxed) as f64 / 1000.0,
            read_wall.as_secs_f64() * 1000.0,
            loaded.as_secs_f64() * 1000.0,
        );
    }
    results.into_iter().map(|r| r.0).collect()
}

/// An unchanged file's stored result holds while every token it stored still names a document
/// and no other token in it may now name one (`names` holds the hashes of every name now).
fn reusable(e: &Entry, known: &(dyn Fn(&str) -> bool + Sync), names: &HashSet<u64>) -> bool {
    if !e.known.iter().all(|(_, t)| known(t)) {
        return false;
    }
    let stored: HashSet<u64> = e.known.iter().map(|(_, t)| token_hash(t)).collect();
    !e.hashes.iter().any(|h| names.contains(h) && !stored.contains(h))
}

fn save(
    file: &Path,
    mut stored: Stored,
    key: String,
    targets: &[(String, Option<BTreeSet<usize>>)],
    results: &[(Extract, Option<Entry>, bool)],
) {
    let mut entries = HashMap::with_capacity(targets.len());
    let mut old = std::mem::take(&mut stored.entries);
    for ((rel, _), (_, entry, reused)) in targets.iter().zip(results) {
        if *reused {
            if let Some(e) = old.remove(rel) {
                entries.insert(rel.clone(), e);
            }
        } else if let Some(e) = entry {
            entries.insert(rel.clone(), e.clone());
        }
    }
    let out = Stored { format: FORMAT.into(), key, entries };
    let Some(dir) = file.parent() else { return };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let tmp = file.with_extension(format!("tmp{}", std::process::id()));
    if let Ok(bytes) = serde_json::to_vec(&out)
        && std::fs::write(&tmp, bytes).is_ok()
    {
        let _ = std::fs::rename(&tmp, file);
    }
}
