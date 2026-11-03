//! S4: a status is one its type allows, with the likely one named for a typo or a word from
//! another tool.

use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::check::rules::support::{key_line, nearest};

pub struct StatusAllowed;

impl Rule for StatusAllowed {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "S4",
            title: "status is one the type allows",
            explain: "Statuses come from one lifecycle (draft, active, superseded, archived, rejected); each type allows a subset. Words from other tools are not accepted but named: accepted means active, proposed means draft, deprecated means superseded. Change status with `spec set <id> status=<value> --write`, which also checks the transition.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if !ctx.scope.covers(i) {
                continue;
            }
            if let Some(v) = d.fm().and_then(|fm| fm.get("status"))
                && !v.is_string()
            {
                let line = key_line(ctx, &d.path, "status");
                let msg = format!("status is {v}, not a string");
                out.push(
                    Finding::error("S4", &d.path, line, msg)
                        .fix_cmd(format!("spec set {} status=<value> --write", d.id)),
                );
                continue;
            }
            let (Some(status), Some(ty)) = (d.status(), ctx.corpus.cfg.registry.get(&d.id.ty)) else { continue };
            if !ty.statuses.contains(&status) {
                let line = key_line(ctx, &d.path, "status");
                out.push(
                    Finding::error(
                        "S4",
                        &d.path,
                        line,
                        format!(
                            "status '{status}' is not allowed for {} (allowed: {}){}",
                            ty.name,
                            ty.statuses.join(", "),
                            status_hint(&status, &ty.statuses)
                        ),
                    )
                    .with_candidates(by_closeness(&status, &ty.statuses)),
                );
            }
        }
    }
}

/// A common word from other tools mapped to the status meant; a hint only, never accepted as-is.
fn status_hint(status: &str, allowed: &[String]) -> String {
    let meant = match status.to_ascii_lowercase().as_str() {
        "accepted" | "approved" | "adopted" | "done" | "implemented" | "current" => "active",
        "proposed" | "proposal" | "wip" | "open" | "pending" => "draft",
        "deprecated" | "replaced" => "superseded",
        "declined" => "rejected",
        "closed" | "obsolete" | "retired" => "archived",
        _ => {
            return match nearest(status, allowed.iter().map(String::as_str)) {
                Some(n) => format!("; did you mean '{n}'?"),
                None => String::new(),
            };
        }
    };
    if allowed.iter().any(|s| s == meant) { format!("; for '{status}' use '{meant}'") } else { String::new() }
}

/// The allowed statuses with a near miss of `status` (a typo) or the word it stands for first,
/// so a picker offers the likely one on top.
fn by_closeness(status: &str, allowed: &[String]) -> Vec<String> {
    let hint = status_hint(status, allowed);
    let first = nearest(status, allowed.iter().map(String::as_str))
        .map(str::to_string)
        .or_else(|| hint.rsplit('\'').nth(1).map(str::to_string));
    let mut out = allowed.to_vec();
    if let Some(f) = first
        && let Some(i) = out.iter().position(|s| *s == f)
    {
        let v = out.remove(i);
        out.insert(0, v);
    }
    out
}
