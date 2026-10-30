//! S8: a field a status requires holds a usable value: not a placeholder, of its declared kind
//! (`[types.<t>] fields = { approvers = "people" }`), and not shared within a `distinct` group.

use super::support::key_line;
use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::graph::Resolution;
use crate::model::doctype::FieldKind;
use crate::model::stamp;
use serde_json::{Map, Value};

pub struct RequiredValues;

/// The value of `key` (or `rel.<kind>`).
fn get<'a>(fm: &'a Map<String, Value>, key: &str) -> Option<&'a Value> {
    match key.strip_prefix("rel.") {
        Some(kind) => fm.get("rel").and_then(|r| r.get(kind)),
        None => fm.get(key),
    }
}

/// The value as a list of strings: a string, or the strings (and numbers) of a list.
fn items(v: &Value) -> Vec<String> {
    match v {
        Value::String(s) => vec![s.trim().to_string()],
        Value::Array(a) => a
            .iter()
            .map(|x| match x {
                Value::String(s) => s.trim().to_string(),
                other => other.to_string(),
            })
            .collect(),
        Value::Null => vec![],
        other => vec![other.to_string()],
    }
}

/// What is wrong with one value for its kind, if anything.
fn bad_kind(ctx: &Ctx, kind: FieldKind, s: &str) -> Option<String> {
    match kind {
        FieldKind::People if s.is_empty() => Some("an empty name".into()),
        FieldKind::People => None,
        FieldKind::Url => {
            let ok = (s.starts_with("https://") || s.starts_with("http://"))
                && s.len() > 8
                && !s.contains(char::is_whitespace);
            (!ok).then(|| format!("'{s}' is not an http(s) URL"))
        }
        FieldKind::Date => (!stamp::is_date_value(s)).then(|| format!("'{s}' is not a YYYY-MM-DD date")),
        FieldKind::Link => {
            let t = s.trim_start_matches("[[").trim_end_matches("]]");
            match ctx.graph.resolve(t) {
                Resolution::Doc { .. } => None,
                _ => Some(format!("'{s}' names no document")),
            }
        }
    }
}

impl Rule for RequiredValues {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "S8",
            title: "a required field has a usable value",
            explain: "Where `require_when` requires a field for a document's status, S6 checks it is there and S8 checks its value: not a placeholder (`[types.<t>] placeholders`, default TBD, Pending, TODO, ?; whole value, any case), of the kind declared in `[types.<t>] fields = { approvers = \"people\", resolution = \"url\", review_by = \"date\", decision = \"link\" }`, and not shared by two fields of a `distinct = [[\"authors\", \"approvers\"]]` group. A warning by default; `[check] fail_on` can make it fail. `spec set <id> <field>=…` gives the real value.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if !ctx.scope.covers(i) {
                continue;
            }
            let (Some(ty), Some(fm), Some(status)) = (ctx.corpus.cfg.registry.get(&d.id.ty), d.fm(), d.status()) else {
                continue;
            };
            let Some(required) = ty.require_when.get(&status) else { continue };
            let line = |k: &str| key_line(ctx, &d.path, k.strip_prefix("rel.").map(|_| "rel").unwrap_or(k));
            let warn = |k: &str, msg: String| {
                Finding::warn("S8", &d.path, line(k), msg)
                    .fix_cmd(format!("spec set {} {k}=…", crate::shell::word(&d.id)))
            };
            for k in required {
                let Some(v) = get(fm, k) else { continue };
                let vals = items(v);
                if let Some(p) = vals.iter().find(|s| ty.placeholders.iter().any(|p| p.eq_ignore_ascii_case(s))) {
                    out.push(warn(k, format!("status '{status}' requires {k}, but '{p}' is a placeholder")));
                    continue;
                }
                if let Some(kind) = ty.kinds.get(k)
                    && let Some(why) = vals.iter().find_map(|s| bad_kind(ctx, *kind, s))
                {
                    let kind = serde_json::to_value(kind).ok().and_then(|v| v.as_str().map(String::from));
                    out.push(warn(k, format!("{k} should be {}: {why}", kind.unwrap_or_default())));
                }
            }
            for group in &ty.distinct {
                if !group.iter().any(|k| required.contains(k)) {
                    continue;
                }
                let mut seen: Vec<(String, &str)> = vec![];
                for k in group {
                    for s in get(fm, k).map(items).unwrap_or_default() {
                        let key = s.to_lowercase();
                        if let Some((_, first)) = seen.iter().find(|(v, f)| *v == key && f != k) {
                            out.push(warn(k, format!("'{s}' is in both {first} and {k}, which must differ")));
                        } else {
                            seen.push((key, k));
                        }
                    }
                }
            }
        }
    }
}
