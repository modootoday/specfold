//! Runs guards: selects them by name, compiles and proves each, scans its range, and holds the
//! hits against its baseline; or measures them to write their baselines.

use super::scan::range::{self, GuardRange};
use super::{compile, prove, scan};
use crate::check::baseline;
use crate::check::finding::Finding;
use crate::config::{Config, GuardDef};
use anyhow::{Result, anyhow, bail};

fn select<'a>(cfg: &'a Config, names: &[String]) -> Result<Vec<&'a GuardDef>> {
    if names.is_empty() {
        return Ok(cfg.raw.guards.iter().collect());
    }
    names
        .iter()
        .map(|n| {
            cfg.raw.guards.iter().find(|g| &g.name == n).ok_or_else(|| {
                // C216: the built-in check runs as `spec guard secrets` with no guard configured.
                let mut have: Vec<&str> = cfg.raw.guards.iter().map(|g| g.name.as_str()).collect();
                have.push("secrets (built-in)");
                anyhow!("no guard named '{n}' (have: {}); next: spec guard secrets", have.join(", "))
            })
        })
        .collect()
}

fn scan_one(cfg: &Config, g: &GuardDef, range: Option<&GuardRange>, net: bool) -> Result<scan::Scanned> {
    let c = compile::compile(cfg, g)?;
    prove::prove(g, &c)?;
    let r = range.cloned().unwrap_or_else(|| GuardRange::parse(&g.range));
    let mut resolved = range::resolve(&cfg.spec_home, &r, &c.bases)?;
    // F3: `--net` judges a range by its end only; versions a later commit takes back are skipped.
    if net {
        resolved.passing.clear();
    }
    scan::run(cfg, g, &c, &resolved)
}

/// Runs the named guards (all of them when `names` is empty). A guard without an explicit range
/// uses its own default; a guard with a baseline reports only hits the baseline does not hold.
pub fn run(cfg: &Config, names: &[String], range: Option<&GuardRange>) -> Result<Vec<Finding>> {
    run_net(cfg, names, range, false)
}

/// [`run`], with `net` judging a range by its net change: what its end holds, not what an
/// intermediate commit wrote and a later one took back (F3).
pub fn run_net(cfg: &Config, names: &[String], range: Option<&GuardRange>, net: bool) -> Result<Vec<Finding>> {
    let mut out = vec![];
    for g in select(cfg, names)? {
        let found = scan_one(cfg, g, range, net)?.findings;
        match &g.baseline {
            Some(rel) => match baseline::load_file(&cfg.spec_home, rel)? {
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
    let guards = select(cfg, names)?;
    // C254: every guard is measured before any baseline is written, and the baseline files of this
    // run never count as uncommitted, so one guard's new file cannot stop the next half way.
    let own: Vec<&String> = guards.iter().filter_map(|g| g.baseline.as_ref()).collect();
    let mut measured = vec![];
    for g in &guards {
        let Some(rel) = &g.baseline else {
            if !names.is_empty() {
                bail!("guard {} has no baseline file; set baseline = \"…\" on it", g.name);
            }
            continue;
        };
        let scanned = scan_one(cfg, g, Some(&GuardRange::All), false)?;
        let b = baseline::measure_with(&cfg.spec_home, &scanned.findings, &[], |p| {
            !own.iter().any(|o| o.as_str() == p) && scanned.files.iter().any(|f| f == p)
        })?;
        measured.push((g.name.clone(), rel.clone(), b));
    }
    let mut written = vec![];
    for (name, rel, b) in measured {
        if write {
            baseline::write_file(&cfg.spec_home, &rel, &b)?;
        }
        written.push((name, rel, b.counts.values().sum()));
    }
    Ok(written)
}
