//! Runs one compiled guard over a resolved range and reports hits as findings (rule U1).

use super::compile::Compiled;
use super::range::{Resolved, Source, read};
use crate::check::finding::{Finding, Severity};
use crate::config::{Config, GuardDef};
use crate::workspace::vcs;
use anyhow::Result;
use std::io::Read;

/// The most a compressed file may expand to before the rest is ignored.
const MAX_INFLATED: u64 = 64 * 1024 * 1024;

fn severity(g: &GuardDef) -> Severity {
    if g.level == "warn" { Severity::Warn } else { Severity::Error }
}

fn hit(g: &GuardDef, path: &str, line: usize, message: String) -> Finding {
    Finding::new("U1", severity(g), path, line, format!("[{}] {message}", g.name))
}

fn scan_text(g: &GuardDef, c: &Compiled, path: &str, text: &str, out: &mut Vec<Finding>) {
    for (n, line) in text.lines().enumerate() {
        for m in c.line_hits(g, path, line) {
            out.push(hit(g, path, n + 1, m));
        }
    }
}

fn inflate(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut out = vec![];
    flate2::read::GzDecoder::new(bytes).take(MAX_INFLATED).read_to_end(&mut out).ok()?;
    Some(out)
}

/// The findings of one guard and the files in its paths it read.
pub struct Scanned {
    pub findings: Vec<Finding>,
    pub files: Vec<String>,
}

pub fn run(cfg: &Config, g: &GuardDef, c: &Compiled, range: &Resolved) -> Result<Scanned> {
    let mut out = vec![];
    let mut files = vec![];
    for path in range.files.iter().filter(|p| c.in_scope(p)) {
        files.push(path.clone());
        if c.denies_file(path) {
            out.push(hit(g, path, 1, "this file may not be here".into()));
        }
        let Some(mut bytes) = read(&cfg.root, &range.source, path) else { continue };
        if let Some(max) = g.max_file_bytes
            && bytes.len() as u64 > max
        {
            out.push(hit(g, path, 1, format!("{} bytes is over {max}", bytes.len())));
        }
        if g.decompress && path.ends_with(".gz") {
            match inflate(&bytes) {
                Some(b) => bytes = b,
                None => out.push(hit(g, path, 1, "cannot be decompressed".into())),
            }
        }
        if bytes.iter().take(8000).any(|b| *b == 0) {
            continue;
        }
        scan_text(g, c, path, &String::from_utf8_lossy(&bytes), &mut out);
    }
    if g.commit_messages
        && let Some(r) = &range.commits
    {
        for commit in vcs::commits(&cfg.root, r).unwrap_or_default() {
            let label = format!("commit {}", &commit.hash[..commit.hash.len().min(10)]);
            scan_text(g, c, &label, &format!("{}\n{}", commit.author, commit.message), &mut out);
        }
    }
    if g.fail_if_empty && matches!(range.source, Source::WorkTree) && files.is_empty() {
        out.push(hit(g, "spec.toml", 1, "scanned no file in its paths; a gate over nothing proves nothing".into()));
    }
    Ok(Scanned { findings: out, files })
}
