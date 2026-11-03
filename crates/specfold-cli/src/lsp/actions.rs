//! What the editor may do about a finding: quick fixes on the buffer, and the one fix command
//! the server runs for the user.

use super::wire::utf16;
use crate::commands::session::Location;
use serde_json::{Value, json};
use specfold_core::check::finding::visible;
use specfold_core::check::{Finding, Fix};
use specfold_core::corpus::overlay;
use std::path::Path;

/// The one command the server executes: a finding's own `--write` fix command.
pub(super) const RUN: &str = "spec.run";

/// One quickfix per diagnostic that carries a replacement, edited against the buffer text.
pub fn code_actions(uri: &str, path: Option<&Path>, params: &Value) -> Vec<Value> {
    let Some(path) = path else { return vec![] };
    let Ok(text) = overlay::read(&Location::process().absolute(path)) else { return vec![] };
    let diags = params.pointer("/context/diagnostics").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut out = vec![];
    for d in &diags {
        let Some(data) = d.get("data") else { continue };
        let line = data.get("line").and_then(Value::as_u64).unwrap_or(0) as usize;
        let edit = |from: &str, to: &str| -> Option<Value> {
            let l = text.lines().nth(line.checked_sub(1)?)?;
            let b = l.find(from)?;
            let (start, end) = (utf16(l, b), utf16(l, b + from.len()));
            let range =
                json!({"start": {"line": line - 1, "character": start}, "end": {"line": line - 1, "character": end}});
            Some(json!({uri: [{"range": range, "newText": to}]}))
        };
        if let (Some(from), Some(to)) =
            (data.get("from").and_then(Value::as_str), data.get("to").and_then(Value::as_str))
            && let Some(changes) = edit(from, to)
        {
            out.push(json!({
                "title": if to.is_empty() {
                    format!("spec: remove `{}`", visible(from))
                } else {
                    format!("spec: replace `{}` with `{}`", visible(from), visible(to))
                },
                "kind": "quickfix",
                "diagnostics": [d],
                "isPreferred": true,
                "edit": {"changes": changes},
            }));
        }
        for c in data.get("choices").and_then(Value::as_array).into_iter().flatten() {
            let (Some(from), Some(to)) = (c.get("from").and_then(Value::as_str), c.get("to").and_then(Value::as_str))
            else {
                continue;
            };
            if let Some(changes) = edit(from, to) {
                out.push(json!({
                    "title": format!("spec: use `{}`", visible(to)),
                    "kind": "quickfix",
                    "diagnostics": [d],
                    "edit": {"changes": changes},
                }));
            }
        }
        if let Some(cmd) = data.get("command").and_then(Value::as_str) {
            out.push(json!({
                "title": format!("spec: run `{cmd}`"),
                "kind": "quickfix",
                "diagnostics": [d],
                "command": {"title": format!("run `{cmd}`"), "command": RUN, "arguments": [cmd]},
            }));
        }
    }
    out
}

/// What a quick fix may change for a finding: one replacement per candidate it lists, written
/// the way `spec fix --pick` would.
pub(super) fn choices(f: &Finding) -> Vec<(String, String)> {
    let each = |from: &str, to: &dyn Fn(&str) -> String| -> Vec<(String, String)> {
        f.candidates.iter().map(|c| (from.to_string(), to(c))).collect()
    };
    match f.rule.as_str() {
        "L1" if f.message.contains("]] is ambiguous") => {
            let Some(target) = f.message.split("]]").next().and_then(|s| s.strip_prefix("[[")) else { return vec![] };
            let base = target.split('#').next().unwrap_or(target);
            each(&format!("[[{base}"), &|c| format!("[[{c}"))
        }
        "L1" => match f.message.split_once(" has no anchor #") {
            Some((_, a)) => each(&format!("#{a}"), &|c| format!("#{c}")),
            None => vec![],
        },
        "L2" => match f.message.strip_prefix("cites the path ") {
            Some(p) => each(p.split(", which is not a document").next().unwrap_or(p), &|c| format!("[[{c}]]")),
            None => vec![],
        },
        "S4" => match f.message.strip_prefix("status '").and_then(|r| r.split_once("' is not allowed")) {
            // The value itself, unless it also occurs inside the key on that line.
            Some((status, _)) if !"status".contains(status) => each(status, &|c| c.to_string()),
            _ => vec![],
        },
        _ => vec![],
    }
}

/// A fix command the editor may run for the user: it writes with `--write` and needs nothing
/// filled in.
pub(super) fn runnable(f: &Finding) -> Option<&str> {
    match &f.fix {
        Some(Fix::Cmd(c)) if c.starts_with("spec ") && c.contains(" --write") && !c.contains('…') => Some(c),
        _ => None,
    }
}

/// Splits a command the way a POSIX shell would for the quoting `spec` itself writes.
pub(super) fn split_words(s: &str) -> Vec<String> {
    let (mut out, mut cur, mut quoted, mut any) = (vec![], String::new(), false, false);
    for ch in s.chars() {
        match ch {
            '\'' => {
                quoted = !quoted;
                any = true;
            }
            ' ' if !quoted => {
                if any || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                any = false;
            }
            '\\' if !quoted => {}
            c => cur.push(c),
        }
    }
    if any || !cur.is_empty() {
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use specfold_core::check::Severity;

    fn finding(rule: &str, message: &str, candidates: &[&str]) -> Finding {
        Finding::new(rule, Severity::Error, "a.md", 1, message)
            .with_candidates(candidates.iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn typos_get_one_replacement_per_candidate() {
        let f = finding("S4", "status 'actve' is not allowed for adr (allowed: draft, active)", &["active", "draft"]);
        assert_eq!(choices(&f)[0], ("actve".to_string(), "active".to_string()));
        let f = finding("L2", "cites the path a/b.md, which is not a document (no such file)", &["adr:x/1"]);
        assert_eq!(choices(&f), vec![("a/b.md".to_string(), "[[adr:x/1]]".to_string())]);
        assert!(choices(&finding("S4", "status 'tat' is not allowed for adr", &["draft"])).is_empty());
    }
}
