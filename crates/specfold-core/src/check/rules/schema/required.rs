//! S6: a status requires the fields its type names for it.

use crate::check::context::{Ctx, key_line};
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};

pub struct RequiredByStatus;

/// Whether `key` (or `rel.<kind>`) holds a non-empty value.
pub(crate) fn has_value(fm: &serde_json::Map<String, serde_json::Value>, key: &str) -> bool {
    let v = match key.strip_prefix("rel.") {
        Some(kind) => fm.get("rel").and_then(|r| r.get(kind)),
        None => fm.get(key),
    };
    match v {
        None | Some(serde_json::Value::Null) => false,
        Some(serde_json::Value::String(s)) => !s.trim().is_empty(),
        Some(serde_json::Value::Array(a)) => !a.is_empty(),
        Some(_) => true,
    }
}

impl Rule for RequiredByStatus {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "S6",
            title: "a status has the fields it requires",
            explain: "A type can require fields once a document reaches a status, through `require_when` in spec.toml (for example a review date for an accepted risk, or rel.depends_on for an active plan). `spec set <id> key=value --write` adds them.",
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
            let missing: Vec<&str> = required.iter().map(String::as_str).filter(|k| !has_value(fm, k)).collect();
            if !missing.is_empty() {
                out.push(
                    Finding::error(
                        "S6",
                        &d.path,
                        key_line(ctx, &d.path, "status"),
                        format!("status '{status}' requires {}", missing.join(", ")),
                    )
                    .fix_cmd(format!(
                        "spec set {} {} --write",
                        crate::suggest::shell::word(&d.id),
                        missing.iter().map(|k| format!("{k}=…")).collect::<Vec<_>>().join(" ")
                    )),
                );
            }
        }
    }
}
