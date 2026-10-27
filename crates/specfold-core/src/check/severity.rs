//! `[check] severity`: a project turns a rule down to a warning, up to an error, or off.

use super::finding::{Finding, Severity};
use crate::corpus::Corpus;

pub fn apply(c: &Corpus, findings: Vec<Finding>) -> Vec<Finding> {
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
