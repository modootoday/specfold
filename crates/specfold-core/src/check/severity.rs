//! `[check] severity`: a project turns a rule down to a warning, up to an error, or off;
//! `[[check.override]]` does the same for files under some paths.

use super::finding::{Finding, Severity};
use crate::config::types::Level;
use crate::corpus::Corpus;
use crate::workspace::owners::matcher;

pub fn apply(c: &Corpus, findings: Vec<Finding>) -> Vec<Finding> {
    let levels = &c.cfg.raw.check.severity;
    let overrides: Vec<_> = c
        .cfg
        .raw
        .check
        .overrides
        .iter()
        .map(|o| (o, o.paths.iter().filter_map(|p| matcher(p)).collect::<Vec<_>>()))
        .collect();
    if levels.is_empty() && overrides.is_empty() {
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
            let path = f.path.clone();
            let hit = overrides.iter().rev().find(|(o, ms)| o.rule == f.rule && ms.iter().any(|m| m.is_match(&path)));
            match hit.map(|(o, _)| o.level) {
                Some(Level::Off) => return None,
                Some(Level::Warn) => f.severity = Severity::Warn,
                Some(Level::Error) => f.severity = Severity::Error,
                None => {}
            }
            Some(f)
        })
        .collect()
}
