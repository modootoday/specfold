//! Runs one guard over a resolved range and reports hits as findings (rule U1).

use super::range::{Resolved, read};
use super::{secrets, terms};
use crate::check::finding::{Finding, Severity};
use crate::config::{Config, GuardDef};
use crate::workspace::owners::{PathMatch, matcher};
use crate::workspace::vcs;
use anyhow::{Result, anyhow};
use regex::Regex;

struct Compiled {
    paths: Vec<PathMatch>,
    exclude: Vec<PathMatch>,
    deny_files: Vec<PathMatch>,
    terms: Option<Regex>,
    regex: Vec<Regex>,
}

fn compile(cfg: &Config, g: &GuardDef) -> Result<Compiled> {
    let regex = g
        .deny_regex
        .iter()
        .map(|r| Regex::new(r).map_err(|e| anyhow!("guard {}: bad regex {r}: {e}", g.name)))
        .collect::<Result<_>>()?;
    Ok(Compiled {
        paths: g.paths.iter().filter_map(|p| matcher(p)).collect(),
        exclude: g.exclude.iter().filter_map(|p| matcher(p)).collect(),
        deny_files: g.deny_files.iter().filter_map(|p| matcher(p)).collect(),
        terms: terms::matcher(&terms::load(cfg, g)?),
        regex,
    })
}

fn severity(g: &GuardDef) -> Severity {
    if g.level == "warn" { Severity::Warn } else { Severity::Error }
}

fn hit(g: &GuardDef, path: &str, line: usize, message: String) -> Finding {
    Finding::new("U1", severity(g), path, line, format!("[{}] {message}", g.name))
}

fn scan_text(g: &GuardDef, c: &Compiled, path: &str, text: &str, out: &mut Vec<Finding>) {
    for (n, line) in text.lines().enumerate() {
        if let Some(m) = c.terms.as_ref().and_then(|t| t.find(line)) {
            out.push(hit(g, path, n + 1, format!("denied term {}", terms::mask(m.as_str()))));
        }
        for re in &c.regex {
            if re.is_match(line) {
                out.push(hit(g, path, n + 1, format!("matches {}", re.as_str())));
            }
        }
        if g.secrets {
            for s in secrets::all() {
                if s.re.is_match(line) {
                    out.push(hit(g, path, n + 1, format!("looks like a {}", s.name)));
                }
            }
        }
    }
}

pub fn run(cfg: &Config, g: &GuardDef, range: &Resolved) -> Result<Vec<Finding>> {
    let c = compile(cfg, g)?;
    let mut out = vec![];
    for path in &range.files {
        let in_scope = (c.paths.is_empty() || c.paths.iter().any(|m| m.is_match(path)))
            && !c.exclude.iter().any(|m| m.is_match(path));
        if !in_scope {
            continue;
        }
        if c.deny_files.iter().any(|m| m.is_match(path)) {
            out.push(hit(g, path, 1, "this file may not be here".into()));
        }
        let Some(bytes) = read(&cfg.root, &range.source, path) else { continue };
        if let Some(max) = g.max_file_bytes
            && bytes.len() as u64 > max
        {
            out.push(hit(g, path, 1, format!("{} bytes is over {max}", bytes.len())));
        }
        if bytes.iter().take(8000).any(|b| *b == 0) {
            continue;
        }
        scan_text(g, &c, path, &String::from_utf8_lossy(&bytes), &mut out);
    }
    if g.commit_messages
        && let Some(r) = &range.commits
    {
        for commit in vcs::commits(&cfg.root, r).unwrap_or_default() {
            let label = format!("commit {}", &commit.hash[..commit.hash.len().min(10)]);
            scan_text(g, &c, &label, &format!("{}\n{}", commit.author, commit.message), &mut out);
        }
    }
    Ok(out)
}
