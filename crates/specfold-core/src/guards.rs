//! Guards: named checks over any file in the repository and over commit messages, such as the
//! gate that keeps internal names out of a public package.

mod cache;
mod code_spans;
mod compile;
mod jslex;
mod pattern;
mod prefilter;
mod prove;
pub mod range;
mod scan;
pub mod secrets;
pub mod terms;

use crate::check::baseline;
use crate::check::finding::Finding;
use crate::config::{Config, GuardDef};
use anyhow::{Result, anyhow, bail};
pub use range::GuardRange;

fn select<'a>(cfg: &'a Config, names: &[String]) -> Result<Vec<&'a GuardDef>> {
    if names.is_empty() {
        return Ok(cfg.raw.guards.iter().collect());
    }
    names
        .iter()
        .map(|n| {
            cfg.raw.guards.iter().find(|g| &g.name == n).ok_or_else(|| {
                let have: Vec<&str> = cfg.raw.guards.iter().map(|g| g.name.as_str()).collect();
                anyhow!("no guard named '{n}' (have: {})", have.join(", "))
            })
        })
        .collect()
}

fn scan_one(cfg: &Config, g: &GuardDef, range: Option<&GuardRange>) -> Result<scan::Scanned> {
    let c = compile::compile(cfg, g)?;
    prove::prove(g, &c)?;
    let r = range.cloned().unwrap_or_else(|| GuardRange::parse(&g.range));
    scan::run(cfg, g, &c, &range::resolve(&cfg.root, &r, &c.bases)?)
}

/// Runs the named guards (all of them when `names` is empty). A guard without an explicit range
/// uses its own default; a guard with a baseline reports only hits the baseline does not hold.
pub fn run(cfg: &Config, names: &[String], range: Option<&GuardRange>) -> Result<Vec<Finding>> {
    let mut out = vec![];
    for g in select(cfg, names)? {
        let found = scan_one(cfg, g, range)?.findings;
        match &g.baseline {
            Some(rel) => match baseline::load_file(&cfg.root, rel)? {
                Some(b) => out.extend(baseline::hold_with(&b, found, &[]).findings),
                None => out.extend(found),
            },
            None => out.extend(found),
        }
    }
    Ok(out)
}

/// Measures each named guard over the whole tree and writes its baseline file.
/// With `write` false, only measures: what `--write-baseline` would record, for its preview.
pub fn write_baselines(cfg: &Config, names: &[String], write: bool) -> Result<Vec<(String, String, usize)>> {
    let mut written = vec![];
    for g in select(cfg, names)? {
        let Some(rel) = &g.baseline else {
            if !names.is_empty() {
                bail!("guard {} has no baseline file; set baseline = \"…\" on it", g.name);
            }
            continue;
        };
        let scanned = scan_one(cfg, g, Some(&GuardRange::All))?;
        let b = baseline::measure_with(&cfg.root, &scanned.findings, &[], |p| scanned.files.iter().any(|f| f == p))?;
        if write {
            baseline::write_file(&cfg.root, rel, &b)?;
        }
        written.push((g.name.clone(), rel.clone(), b.counts.values().sum()));
    }
    Ok(written)
}

mod blobs;
#[cfg(test)]
mod tests;
pub use blobs::{builtin_secrets, reads_content, scan_blobs};
