//! Findings in the formats CI systems read: GitHub workflow commands, SARIF 2.1.0, GitLab Code
//! Quality and reviewdog's rdjson. Each is a whole document on standard output.

use serde_json::{Value, json};
use specfold_core::check::{Finding, Fix, Severity, rules::catalog};
use std::path::Path;

pub const FORMATS: &[&str] = &["text", "github", "sarif", "gitlab", "rdjson"];

/// FNV-1a over rule, path and message: stable across runs and line moves, unlike line numbers.
pub fn fingerprint(f: &Finding) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in f.rule.bytes().chain([0]).chain(f.path.bytes()).chain([0]).chain(f.message.bytes()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// One fingerprint per finding, unique within the run: a repeat of the same rule, path and
/// message gets `-2`, `-3`… in order, so GitLab and code scanning keep both instead of folding them.
pub fn fingerprints(findings: &[Finding]) -> Vec<String> {
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    findings
        .iter()
        .map(|f| {
            let base = fingerprint(f);
            let n = seen.entry(base.clone()).or_insert(0);
            *n += 1;
            if *n == 1 { base } else { format!("{base}-{n}") }
        })
        .collect()
}

fn escape_data(s: &str) -> String {
    s.replace('%', "%25").replace('\r', "%0D").replace('\n', "%0A")
}

fn escape_property(s: &str) -> String {
    escape_data(s).replace(':', "%3A").replace(',', "%2C")
}

fn fix_text(f: &Finding) -> Option<String> {
    match &f.fix {
        Some(Fix::Cmd(c)) => Some(format!("fix: {c}")),
        Some(Fix::Replace { from, to, .. }) => Some(format!("fix: replace `{from}` with `{to}` (spec fix --write)")),
        None => None,
    }
}

fn message(f: &Finding) -> String {
    match fix_text(f) {
        Some(t) => format!("{}\n{t}", f.message),
        None => f.message.clone(),
    }
}

/// `::error file=…,line=…,title=…::message`, one per finding, then a plain count.
pub fn github(findings: &[Finding], errors: usize, warnings: usize) -> String {
    let mut s = String::new();
    for f in findings {
        let level = if f.severity == Severity::Error { "error" } else { "warning" };
        let mut props = vec![format!("file={}", escape_property(&f.path))];
        if f.line > 0 {
            props.push(format!("line={}", f.line));
        }
        props.push(format!("title={}", escape_property(&format!("spec {}", f.rule))));
        s.push_str(&format!("::{level} {}::{}\n", props.join(","), escape_data(&message(f))));
    }
    s.push_str(&format!("spec check: {errors} error(s), {warnings} warning(s)\n"));
    s
}

/// The step summary: a Markdown table GitHub shows on the run page.
pub fn github_summary(findings: &[Finding], errors: usize, warnings: usize) -> String {
    let mut s = format!("### spec check\n\n{errors} error(s), {warnings} warning(s)\n");
    if !findings.is_empty() {
        s.push_str("\n| rule | severity | location | message |\n| --- | --- | --- | --- |\n");
        for f in findings.iter().take(200) {
            let loc = if f.line > 0 { format!("{}:{}", f.path, f.line) } else { f.path.clone() };
            let sev = if f.severity == Severity::Error { "error" } else { "warning" };
            let msg = f.message.replace('|', "\\|").replace('\n', " ");
            s.push_str(&format!("| {} | {sev} | `{loc}` | {msg} |\n", f.rule));
        }
        if findings.len() > 200 {
            s.push_str(&format!("\n{} more in the log\n", findings.len() - 200));
        }
    }
    s
}

pub fn sarif(findings: &[Finding]) -> String {
    let mut rules: Vec<Value> = vec![];
    let mut ids: Vec<String> = vec![];
    for r in catalog() {
        let i = r.info();
        ids.push(i.id.to_string());
        rules.push(json!({
            "id": i.id,
            "name": i.title,
            "shortDescription": {"text": i.title},
            "fullDescription": {"text": i.explain},
            "help": {"text": format!("{}\n\nspec explain {}", i.explain, i.id)},
        }));
    }
    for f in findings {
        if !ids.contains(&f.rule) {
            ids.push(f.rule.clone());
            rules.push(json!({"id": f.rule, "shortDescription": {"text": f.rule}}));
        }
    }
    let prints = fingerprints(findings);
    let results: Vec<Value> = findings
        .iter()
        .zip(&prints)
        .map(|(f, print)| {
            let mut physical = json!({"artifactLocation": {"uri": f.path, "uriBaseId": "%SRCROOT%"}});
            if f.line > 0 {
                physical["region"] = json!({"startLine": f.line});
            }
            json!({
                "ruleId": f.rule,
                "ruleIndex": ids.iter().position(|i| i == &f.rule).unwrap_or(0),
                "level": if f.severity == Severity::Error { "error" } else { "warning" },
                "message": {"text": message(f)},
                "locations": [{"physicalLocation": physical}],
                "partialFingerprints": {"specfold/v1": print},
            })
        })
        .collect();
    let doc = json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {"driver": {"name": "specfold", "version": env!("CARGO_PKG_VERSION"), "rules": rules}},
            "originalUriBaseIds": {"%SRCROOT%": {"uri": "./"}},
            "results": results,
        }],
    });
    format!("{}\n", serde_json::to_string_pretty(&doc).unwrap_or_default())
}

pub fn gitlab(findings: &[Finding]) -> String {
    let prints = fingerprints(findings);
    let list: Vec<Value> = findings
        .iter()
        .zip(&prints)
        .map(|(f, print)| {
            json!({
                "description": format!("{}: {}", f.rule, message(f)),
                "check_name": f.rule,
                "fingerprint": print,
                "severity": if f.severity == Severity::Error { "major" } else { "minor" },
                "location": {"path": f.path, "lines": {"begin": f.line.max(1)}},
            })
        })
        .collect();
    format!("{}\n", serde_json::to_string_pretty(&Value::Array(list)).unwrap_or_default())
}

/// reviewdog Diagnostic Format. A replacement fix becomes a suggestion on its exact columns
/// (1-based, counted in UTF-8 bytes as rdjson specifies), so reviewdog can offer it in review.
pub fn rdjson(findings: &[Finding], root: &Path) -> String {
    let diagnostics: Vec<Value> = findings
        .iter()
        .map(|f| {
            let mut d = json!({
                "message": message(f),
                "location": {"path": f.path},
                "severity": if f.severity == Severity::Error { "ERROR" } else { "WARNING" },
                "code": {"value": f.rule},
            });
            if f.line > 0 {
                d["location"]["range"] = json!({"start": {"line": f.line}});
            }
            if let Some(Fix::Replace { path, line, from, to }) = &f.fix
                && let Some(col) = column_of(root, path, *line, from)
            {
                d["suggestions"] = json!([{
                    "range": {"start": {"line": line, "column": col}, "end": {"line": line, "column": col + from.len()}},
                    "text": to,
                }]);
            }
            d
        })
        .collect();
    let doc = json!({"source": {"name": "specfold"}, "diagnostics": diagnostics});
    format!("{}\n", serde_json::to_string_pretty(&doc).unwrap_or_default())
}

fn column_of(root: &Path, path: &str, line: usize, from: &str) -> Option<usize> {
    let text = std::fs::read_to_string(root.join(path)).ok()?;
    let l = text.lines().nth(line.checked_sub(1)?)?;
    l.find(from).map(|b| b + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_escapes_properties_and_data() {
        let f = Finding::new("L1", Severity::Error, "a,b:c.md", 3, "50% done\nnext");
        let s = github(&[f], 1, 0);
        assert!(s.starts_with("::error file=a%2Cb%3Ac.md,line=3,title=spec L1::50%25 done%0Anext\n"), "{s}");
    }

    #[test]
    fn repeated_findings_get_numbered_fingerprints() {
        let a = Finding::new("L1", Severity::Error, "a.md", 3, "m");
        let b = Finding::new("L1", Severity::Error, "a.md", 9, "m");
        let c = Finding::new("L2", Severity::Error, "a.md", 9, "m");
        let p = fingerprints(&[a.clone(), b, c, a.clone()]);
        assert_eq!(p[0], fingerprint(&a));
        assert_eq!(p[1], format!("{}-2", fingerprint(&a)));
        assert_eq!(p[3], format!("{}-3", fingerprint(&a)));
        assert!(!p[2].contains('-'));
    }

    #[test]
    fn fingerprints_ignore_the_line() {
        let a = Finding::new("L1", Severity::Error, "a.md", 3, "m");
        let b = Finding::new("L1", Severity::Error, "a.md", 9, "m");
        assert_eq!(fingerprint(&a), fingerprint(&b));
        assert_ne!(fingerprint(&a), fingerprint(&Finding::new("L2", Severity::Error, "a.md", 3, "m")));
    }
}
