//! A committed baseline of known findings, so a project can adopt specfold on an existing pile:
//! what is already there is accepted, anything new fails, and the counts may only go down.
//! Paths under `[check] strict` never get a baseline.

use super::finding::{Finding, Severity};
use crate::corpus::Corpus;
use crate::workspace::owners::{PathMatch, matcher};
use crate::workspace::vcs;
use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SCHEMA: &str = "spec.baseline.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Baseline {
    pub schema: String,
    /// The commit whose documents were measured; it must stay in HEAD's history.
    pub measured_on: String,
    /// Error counts keyed `RULE<TAB>path`.
    pub counts: BTreeMap<String, usize>,
}

/// The outcome of holding findings against a baseline.
#[derive(Debug, Default)]
pub struct Held {
    /// Findings the baseline does not cover; these decide the exit code.
    pub findings: Vec<Finding>,
    /// Errors accepted because the baseline lists them.
    pub known: usize,
    /// Keys whose count fell below the baseline: (key, baseline, now).
    pub lowerable: Vec<(String, usize, usize)>,
}

fn key(f: &Finding) -> String {
    format!("{}\t{}", f.rule, f.path)
}

fn strict_matchers(c: &Corpus) -> Vec<PathMatch> {
    c.cfg.raw.check.strict.iter().filter_map(|p| matcher(p)).collect()
}

pub fn load(c: &Corpus) -> Result<Option<Baseline>> {
    let Some(rel) = &c.cfg.raw.check.baseline else { return Ok(None) };
    let path = c.cfg.root.join(rel);
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path).with_context(|| format!("cannot read {rel}"))?;
    let b: Baseline = serde_json::from_str(&text).with_context(|| format!("{rel} is not a {SCHEMA} file"))?;
    if b.schema != SCHEMA {
        bail!("{rel} has schema {}, expected {SCHEMA}", b.schema);
    }
    match vcs::is_ancestor_of_head(&c.cfg.root, &b.measured_on) {
        Some(true) => Ok(Some(b)),
        Some(false) => bail!(
            "{rel} was measured on {}, which is not in HEAD's history; measure again with spec check --write-baseline",
            b.measured_on
        ),
        None => bail!("{rel} names {}, which git does not know", b.measured_on),
    }
}

pub fn hold(c: &Corpus, b: &Baseline, findings: Vec<Finding>) -> Held {
    let strict = strict_matchers(c);
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let mut held = Held::default();
    for f in findings {
        if f.severity != Severity::Error || strict.iter().any(|m| m.is_match(&f.path)) {
            held.findings.push(f);
            continue;
        }
        let k = key(&f);
        let n = seen.entry(k.clone()).or_default();
        *n += 1;
        if *n <= b.counts.get(&k).copied().unwrap_or(0) {
            held.known += 1;
        } else {
            held.findings.push(f);
        }
    }
    for (k, &allowed) in &b.counts {
        let now = seen.get(k).copied().unwrap_or(0);
        if now < allowed {
            held.lowerable.push((k.clone(), allowed, now));
        }
    }
    held
}

/// Measures the errors of a full run into a baseline. Refuses when a measured document differs
/// from HEAD, because the baseline names a commit and must describe that commit's bytes.
pub fn measure(c: &Corpus, findings: &[Finding]) -> Result<Baseline> {
    let root = &c.cfg.root;
    let head = vcs::head(root).ok_or_else(|| anyhow!("a baseline needs a git repository with a commit"))?;
    let changed = vcs::changed_since(root, "HEAD").unwrap_or_default();
    let dirty: Vec<&String> = changed.iter().filter(|p| c.docs.iter().any(|d| &d.path == *p)).take(10).collect();
    if !dirty.is_empty() {
        let list: Vec<&str> = dirty.iter().map(|s| s.as_str()).collect();
        bail!("commit these documents first, the baseline names HEAD: {}", list.join(", "));
    }
    let strict = strict_matchers(c);
    let mut counts = BTreeMap::new();
    for f in findings.iter().filter(|f| f.severity == Severity::Error && !strict.iter().any(|m| m.is_match(&f.path))) {
        *counts.entry(key(f)).or_default() += 1;
    }
    Ok(Baseline { schema: SCHEMA.into(), measured_on: head, counts })
}

pub fn write(c: &Corpus, b: &Baseline) -> Result<String> {
    let rel = c.cfg.raw.check.baseline.clone().ok_or_else(|| anyhow!("set [check] baseline in spec.toml first"))?;
    let path = c.cfg.root.join(&rel);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(b)? + "\n")?;
    Ok(rel)
}

/// Applies `[check] severity`: a rule turned off is dropped, a rule set to warn or error is
/// reported at that level.
pub fn apply_severity(c: &Corpus, findings: Vec<Finding>) -> Vec<Finding> {
    let levels = &c.cfg.raw.check.severity;
    if levels.is_empty() {
        return findings;
    }
    findings
        .into_iter()
        .filter_map(|mut f| {
            match levels.get(&f.rule).map(String::as_str) {
                Some("off") => return None,
                Some("warn") => f.severity = Severity::Warn,
                Some("error") => f.severity = Severity::Error,
                _ => {}
            }
            Some(f)
        })
        .collect()
}
