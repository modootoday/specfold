//! How findings appear: one line each with the location first, then the fix on its own line.

use super::envelope::Output;
use serde_json::to_value;
use specfold_core::check::finding::visible;
use specfold_core::check::{Finding, Fix, Severity};

pub fn line(f: &Finding) -> String {
    let sev = match f.severity {
        Severity::Error => "error",
        Severity::Warn => "warn",
    };
    let loc = if f.line > 0 { format!("{}:{}", f.path, f.line) } else { f.path.clone() };
    let mut s = format!("{loc}  {} {sev}  {}", f.rule, visible(&f.message));
    if !f.candidates.is_empty() {
        s.push_str(&format!("\n  candidates: {}", f.candidates.join(", ")));
    }
    match &f.fix {
        Some(Fix::Cmd(c)) => s.push_str(&format!("\n  fix: {c}")),
        Some(Fix::Replace { from, to, .. }) if to.is_empty() => {
            s.push_str(&format!("\n  fix: remove `{}`  (spec fix --write {loc})", visible(from)))
        }
        Some(Fix::Replace { from, to, .. }) => s.push_str(&format!(
            "\n  fix: replace `{}` with `{}`  (spec fix --write {loc})",
            visible(from),
            visible(to)
        )),
        None => {}
    }
    s
}

pub fn push_all(mut o: Output, findings: &[Finding]) -> Output {
    for f in findings {
        o = o.item(to_value(f).unwrap_or_default(), line(f));
    }
    o
}

pub fn counts(findings: &[Finding]) -> (usize, usize) {
    let errors = findings.iter().filter(|f| f.severity == Severity::Error).count();
    (errors, findings.len() - errors)
}
