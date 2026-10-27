//! A committed baseline of known findings, so a project can adopt specfold on an existing pile:
//! what is already there is accepted, anything new fails, and the counts may only go down.
//! Used by `spec check` ([check] baseline) and by each guard (its `baseline`); strict paths never
//! get one.

use super::finding::{Finding, Severity};
use crate::corpus::Corpus;
use crate::workspace::owners::{PathMatch, matcher};
use crate::workspace::vcs;
use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

pub const SCHEMA: &str = "spec.baseline.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Baseline {
    pub schema: String,
    /// The commit whose files were measured, for the record. A rebase or a cherry-pick onto a
    /// newer main renames it, so it is not checked: the counts are keyed by rule and path.
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

fn counted(f: &Finding, strict: &[PathMatch]) -> bool {
    f.severity == Severity::Error && !strict.iter().any(|m| m.is_match(&f.path))
}

pub fn matchers(patterns: &[String]) -> Vec<PathMatch> {
    patterns.iter().filter_map(|p| matcher(p)).collect()
}

/// Reads `rel` under `root`; None when it does not exist yet.
pub fn load_file(root: &Path, rel: &str) -> Result<Option<Baseline>> {
    let path = root.join(rel);
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path).with_context(|| format!("cannot read {rel}"))?;
    let b: Baseline = serde_json::from_str(&text).with_context(|| format!("{rel} is not a {SCHEMA} file"))?;
    if b.schema != SCHEMA {
        bail!("{rel} has schema {}, expected {SCHEMA}", b.schema);
    }
    Ok(Some(b))
}

pub fn hold_with(b: &Baseline, findings: Vec<Finding>, strict: &[PathMatch]) -> Held {
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let mut held = Held::default();
    for f in findings {
        if !counted(&f, strict) {
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

/// Measures errors into a baseline. Refuses when a measured file differs from HEAD, because the
/// baseline names a commit and must describe that commit's bytes.
pub fn measure_with(
    root: &Path,
    findings: &[Finding],
    strict: &[PathMatch],
    measured: impl Fn(&str) -> bool,
) -> Result<Baseline> {
    let head = vcs::head(root).ok_or_else(|| anyhow!("a baseline needs a git repository with a commit"))?;
    let changed = vcs::changed_since(root, "HEAD").unwrap_or_default();
    let dirty: Vec<&str> = changed.iter().map(String::as_str).filter(|p| measured(p)).take(10).collect();
    if !dirty.is_empty() {
        bail!("commit these files first, the baseline names HEAD: {}", dirty.join(", "));
    }
    let mut counts = BTreeMap::new();
    for f in findings.iter().filter(|f| counted(f, strict)) {
        *counts.entry(key(f)).or_default() += 1;
    }
    Ok(Baseline { schema: SCHEMA.into(), measured_on: head, counts })
}

pub fn write_file(root: &Path, rel: &str, b: &Baseline) -> Result<()> {
    let path = root.join(rel);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(b)? + "\n")?;
    Ok(())
}

pub fn load(c: &Corpus) -> Result<Option<Baseline>> {
    match &c.cfg.raw.check.baseline {
        Some(rel) => load_file(&c.cfg.root, rel),
        None => Ok(None),
    }
}

pub fn hold(c: &Corpus, b: &Baseline, findings: Vec<Finding>) -> Held {
    hold_with(b, findings, &matchers(&c.cfg.raw.check.strict))
}

pub fn measure(c: &Corpus, findings: &[Finding]) -> Result<Baseline> {
    let strict = matchers(&c.cfg.raw.check.strict);
    measure_with(&c.cfg.root, findings, &strict, |p| c.docs.iter().any(|d| d.path == p))
}

pub fn write(c: &Corpus, b: &Baseline) -> Result<String> {
    let rel = c.cfg.raw.check.baseline.clone().ok_or_else(|| anyhow!("set [check] baseline in spec.toml first"))?;
    write_file(&c.cfg.root, &rel, b)?;
    Ok(rel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_baseline_from_a_commit_a_replay_renamed_still_loads() {
        let tmp = tempfile::tempdir().unwrap();
        let b = Baseline {
            schema: SCHEMA.into(),
            measured_on: "0000000000000000000000000000000000000000".into(),
            counts: BTreeMap::from([("L1\ta.md".to_string(), 2)]),
        };
        write_file(tmp.path(), "spec-baseline.json", &b).unwrap();
        let loaded = load_file(tmp.path(), "spec-baseline.json").unwrap().unwrap();
        assert_eq!(loaded.counts["L1\ta.md"], 2);
    }
}
