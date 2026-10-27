//! A guard proves itself before it runs: every fixture line it must flag is flagged and every
//! line it must pass is passed. A pattern that silently stopped matching would otherwise turn
//! the gate green over everything.

use super::scan::Compiled;
use crate::config::GuardDef;
use anyhow::{Result, bail};

/// The path fixtures are judged as, so per-path exemptions do not apply to them.
const FIXTURE_PATH: &str = "<fixture>";

pub fn prove(g: &GuardDef, c: &Compiled) -> Result<()> {
    for p in &c.patterns {
        for line in &p.must_hit {
            if p.find(line).is_none() {
                bail!("guard {}: pattern {} does not match its must_hit fixture", g.name, p.label);
            }
        }
        for line in &p.must_pass {
            if p.find(line).is_some() {
                bail!("guard {}: pattern {} matches its must_pass fixture", g.name, p.label);
            }
        }
    }
    for (i, line) in g.fixtures.must_hit.iter().enumerate() {
        if c.line_hits(g, FIXTURE_PATH, line).is_empty() {
            bail!("guard {}: fixtures.must_hit[{i}] is not flagged", g.name);
        }
    }
    for (i, line) in g.fixtures.must_pass.iter().enumerate() {
        if !c.line_hits(g, FIXTURE_PATH, line).is_empty() {
            bail!("guard {}: fixtures.must_pass[{i}] is flagged", g.name);
        }
    }
    Ok(())
}
