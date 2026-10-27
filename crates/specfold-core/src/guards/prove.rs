//! A guard proves itself before it runs: every fixture it must flag is flagged and every one it
//! must pass is passed. A pattern that silently stopped matching would otherwise turn the gate
//! green over everything.

use super::compile::Compiled;
use super::pattern::Pattern;
use crate::config::GuardDef;
use anyhow::{Result, bail};

/// The path fixtures are judged as, so per-path exemptions do not apply to them.
const FIXTURE_PATH: &str = "<fixture>";

fn prove_pattern(g: &GuardDef, p: &Pattern, matches: impl Fn(&str) -> bool) -> Result<()> {
    if let Some(line) = p.must_hit.iter().find(|l| !matches(l)) {
        bail!("guard {}: pattern {} does not match its must_hit fixture {line:?}", g.name, p.label);
    }
    if let Some(line) = p.must_pass.iter().find(|l| matches(l)) {
        bail!("guard {}: pattern {} matches its must_pass fixture {line:?}", g.name, p.label);
    }
    Ok(())
}

pub fn prove(g: &GuardDef, c: &Compiled) -> Result<()> {
    for p in &c.patterns {
        prove_pattern(g, p, |l| p.trips(l))?;
    }
    for p in &c.path_patterns {
        prove_pattern(g, p, |l| p.re.is_match(l).unwrap_or(false))?;
    }
    for (i, line) in g.fixtures.must_hit.iter().enumerate() {
        if c.text_hits(g, FIXTURE_PATH, line).is_empty() {
            bail!("guard {}: fixtures.must_hit[{i}] is not flagged", g.name);
        }
    }
    for (i, line) in g.fixtures.must_pass.iter().enumerate() {
        if !c.text_hits(g, FIXTURE_PATH, line).is_empty() {
            bail!("guard {}: fixtures.must_pass[{i}] is flagged", g.name);
        }
    }
    Ok(())
}
