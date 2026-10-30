//! Runs one compiled guard over a resolved range and reports hits as findings (rule U1).

use super::cache::GuardCache;
use super::compile::Compiled;
use super::range::{Resolved, Source, read};
use crate::check::finding::{Finding, Severity};
use crate::config::{Config, GuardDef};
use crate::workspace::vcs;
use anyhow::Result;
use rayon::prelude::*;
use std::io::Read;

/// The most a compressed file may expand to before the rest is ignored.
const MAX_INFLATED: u64 = 16 * 1024 * 1024;
const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];

fn severity(g: &GuardDef) -> Severity {
    if g.level == "warn" { Severity::Warn } else { Severity::Error }
}

fn hit(g: &GuardDef, path: &str, line: usize, message: String) -> Finding {
    Finding::new("U1", severity(g), path, line, format!("[{}] {message}", g.name))
}

fn scan_text(g: &GuardDef, c: &Compiled, path: &str, text: &str, out: &mut Vec<Finding>) {
    for (line, message) in c.text_hits(g, path, text) {
        out.push(hit(g, path, line, message));
    }
}

fn inflate(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut out = vec![];
    flate2::read::GzDecoder::new(bytes).take(MAX_INFLATED).read_to_end(&mut out).ok()?;
    Some(out)
}

/// The text of a file as a reader of the published bytes would see it: UTF-8 when it is, Latin-1
/// otherwise. None for a binary file the guard does not read.
fn as_text(g: &GuardDef, bytes: &[u8]) -> Option<String> {
    if let Ok(s) = std::str::from_utf8(bytes)
        && !s.contains('\0')
    {
        return Some(s.to_string());
    }
    if !g.read_binary {
        return None;
    }
    Some(bytes.iter().map(|&b| b as char).collect())
}

/// The findings of one guard and the files in its paths it read.
pub struct Scanned {
    pub findings: Vec<Finding>,
    pub files: Vec<String>,
}

/// Everything one file trips: its presence, its path and its content.
fn scan_file(cfg: &Config, g: &GuardDef, c: &Compiled, range: &Resolved, path: &str) -> Vec<Finding> {
    let mut out = vec![];
    if c.denies_file(path) {
        out.push(hit(g, path, 1, "this file may not be here".into()));
    }
    for m in c.path_hits(path) {
        out.push(hit(g, path, 1, m));
    }
    let Some(bytes) = read(&cfg.root, &range.source, path) else { return out };
    out.extend(scan_bytes(g, c, path, bytes));
    out
}

/// What the content of one file trips: its size, its compression and its text.
pub fn scan_bytes(g: &GuardDef, c: &Compiled, path: &str, mut bytes: Vec<u8>) -> Vec<Finding> {
    let mut out = vec![];
    if let Some(max) = g.max_file_bytes
        && bytes.len() as u64 > max
    {
        out.push(hit(g, path, 1, format!("{} bytes is over {max}", bytes.len())));
    }
    if g.decompress && bytes.starts_with(&GZIP_MAGIC) {
        match inflate(&bytes) {
            Some(b) => bytes = b,
            None => out.push(hit(g, path, 1, "cannot be decompressed".into())),
        }
    }
    if let Some(text) = as_text(g, &bytes) {
        scan_text(g, c, path, &text, &mut out);
    }
    out
}

/// What a file outside the tree trips under its label: its presence, its label and its content.
pub fn scan_blob(g: &GuardDef, c: &Compiled, label: &str, bytes: Vec<u8>) -> Vec<Finding> {
    let mut out = vec![];
    if c.denies_file(label) {
        out.push(hit(g, label, 1, "this file may not be here".into()));
    }
    for m in c.path_hits(label) {
        out.push(hit(g, label, 1, m));
    }
    out.extend(scan_bytes(g, c, label, bytes));
    out
}

/// Half the machine at most, so a guard on a busy host leaves room for everything else.
/// `RAYON_NUM_THREADS` overrides it.
fn workers() -> rayon::ThreadPool {
    let half = std::thread::available_parallelism().map(|n| (n.get() / 2).max(1)).unwrap_or(1);
    let n = std::env::var("RAYON_NUM_THREADS").ok().and_then(|v| v.parse().ok()).unwrap_or(half);
    rayon::ThreadPoolBuilder::new().num_threads(n).build().unwrap_or_else(|_| {
        rayon::ThreadPoolBuilder::new().num_threads(1).build().expect("a one-thread pool always builds")
    })
}

pub fn run(cfg: &Config, g: &GuardDef, c: &Compiled, range: &Resolved) -> Result<Scanned> {
    let mut out = vec![];
    let mut files = vec![];
    for p in &c.missing {
        out.push(hit(g, p, 1, "a published path is not present, so nothing reads it".into()));
    }
    let whole_tree = matches!(range.source, Source::WorkTree);
    let mut cache = whole_tree.then(|| GuardCache::open(&cfg.root, g, c.terms_source()));
    let mut misses = vec![];
    // spec.toml holds the guard's own patterns, so scanning it would always hit itself.
    for path in range.files.iter().filter(|p| p.as_str() != "spec.toml" && c.in_scope(p)) {
        files.push(path.clone());
        match cache.as_mut().and_then(|k| k.lookup(&cfg.root, path)) {
            Some(found) => out.extend(found),
            None => misses.push(path.as_str()),
        }
    }
    let scanned: Vec<(&str, Vec<Finding>)> =
        workers().install(|| misses.par_iter().map(|p| (*p, scan_file(cfg, g, c, range, p))).collect());
    for (path, found) in scanned {
        if let Some(k) = cache.as_mut() {
            k.put(&cfg.root, path, &found);
        }
        out.extend(found);
    }
    if let Some(k) = cache {
        k.save();
    }
    if g.commit_messages
        && let Some(r) = &range.commits
    {
        for commit in vcs::commits(&cfg.root, r).unwrap_or_default() {
            let label = format!("commit {}", &commit.hash[..commit.hash.len().min(10)]);
            scan_text(g, c, &label, &format!("{}\n{}", commit.author, commit.message), &mut out);
        }
    }
    if g.fail_if_empty && matches!(range.source, Source::WorkTree) {
        if files.is_empty() {
            out.push(hit(g, "spec.toml", 1, "scanned no file in its paths; a gate over nothing proves nothing (fix the paths, or set fail_if_empty = false until the first file exists)".into()));
        }
        // An empty checkout of a published path (a submodule never fetched) reads as clean.
        for p in c.published.iter().filter(|p| !files.iter().any(|f| f.starts_with(&format!("{p}/")))) {
            out.push(hit(g, p, 1, "a published path yielded no file; is it checked out?".into()));
        }
    }
    Ok(Scanned { findings: out, files })
}
