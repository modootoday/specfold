//! Runs one guard over a resolved range and reports hits as findings (rule U1).

use super::range::{Resolved, Source, read};
use super::{secrets, terms};
use crate::check::finding::{Finding, Severity};
use crate::config::{Config, GuardDef};
use crate::workspace::owners::{PathMatch, matcher};
use crate::workspace::vcs;
use anyhow::{Result, anyhow};
use regex::Regex;
use std::io::Read;
use std::sync::OnceLock;

/// The most a compressed file may expand to before the rest is ignored.
const MAX_INFLATED: u64 = 64 * 1024 * 1024;

pub struct Pattern {
    pub re: Regex,
    pub label: String,
    pub not_in_url: bool,
    pub exempt: Vec<PathMatch>,
    pub must_hit: Vec<String>,
    pub must_pass: Vec<String>,
}

pub struct Compiled {
    paths: Vec<PathMatch>,
    exclude: Vec<PathMatch>,
    deny_files: Vec<PathMatch>,
    terms: Option<Regex>,
    pub patterns: Vec<Pattern>,
    allow: Vec<String>,
}

pub fn compile(cfg: &Config, g: &GuardDef) -> Result<Compiled> {
    let mut patterns = vec![];
    for rule in &g.deny_regex {
        let s = rule.spec();
        let re = Regex::new(&s.re).map_err(|e| anyhow!("guard {}: bad regex {}: {e}", g.name, s.re))?;
        patterns.push(Pattern {
            label: s.label.clone().unwrap_or_else(|| s.re.clone()),
            re,
            not_in_url: s.not_in_url,
            exempt: s.exempt.iter().filter_map(|p| matcher(p)).collect(),
            must_hit: s.must_hit,
            must_pass: s.must_pass,
        });
    }
    Ok(Compiled {
        paths: g.paths.iter().filter_map(|p| matcher(p)).collect(),
        exclude: g.exclude.iter().filter_map(|p| matcher(p)).collect(),
        deny_files: g.deny_files.iter().filter_map(|p| matcher(p)).collect(),
        terms: terms::matcher(&terms::load(cfg, g)?),
        patterns,
        allow: g.allow.iter().map(|a| a.to_lowercase()).collect(),
    })
}

fn url_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"[a-zA-Z][a-zA-Z0-9+.-]*://[^\s"'<>)\]]+"#).unwrap())
}

impl Pattern {
    /// The first match on `line` that does not sit inside a URL (when asked).
    pub fn find<'a>(&self, line: &'a str) -> Option<&'a str> {
        let urls: Vec<(usize, usize)> =
            if self.not_in_url { url_re().find_iter(line).map(|m| (m.start(), m.end())).collect() } else { vec![] };
        self.re
            .find_iter(line)
            .find(|m| !urls.iter().any(|(s, e)| m.start() >= *s && m.end() <= *e))
            .map(|m| m.as_str())
    }
}

impl Compiled {
    fn allowed(&self, text: &str) -> bool {
        let t = text.to_lowercase();
        self.allow.iter().any(|a| t.contains(a.as_str()))
    }

    /// What one line of `path` trips, as messages; terms are masked.
    pub fn line_hits(&self, g: &GuardDef, path: &str, line: &str) -> Vec<String> {
        let mut out = vec![];
        if let Some(m) = self.terms.as_ref().and_then(|t| t.find(line))
            && !self.allowed(m.as_str())
        {
            out.push(format!("denied term {}", terms::mask(m.as_str())));
        }
        for p in self.patterns.iter().filter(|p| !p.exempt.iter().any(|m| m.is_match(path))) {
            if let Some(m) = p.find(line)
                && !self.allowed(m)
            {
                out.push(format!("matches {}", p.label));
            }
        }
        if g.secrets {
            for s in secrets::all() {
                if s.re.find(line).is_some_and(|m| !self.allowed(m.as_str())) {
                    out.push(format!("looks like a {}", s.name));
                }
            }
        }
        out
    }

    fn in_scope(&self, path: &str) -> bool {
        (self.paths.is_empty() || self.paths.iter().any(|m| m.is_match(path)))
            && !self.exclude.iter().any(|m| m.is_match(path))
    }
}

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

/// The findings of one guard and how many files in its paths it read.
pub struct Scanned {
    pub findings: Vec<Finding>,
    pub files: Vec<String>,
}

pub fn run(cfg: &Config, g: &GuardDef, c: &Compiled, range: &Resolved) -> Result<Scanned> {
    let mut out = vec![];
    let mut files = vec![];
    for path in range.files.iter().filter(|p| c.in_scope(p)) {
        files.push(path.clone());
        if c.deny_files.iter().any(|m| m.is_match(path)) {
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
