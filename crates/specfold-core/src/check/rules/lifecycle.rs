//! T rules: supersede claims pair up, statuses only move forward, sealed bodies stay sealed,
//! and an edited body moves its `updated` date.

use super::support::key_line;
use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::graph::query;
use crate::markdown::frontmatter::{self, Parsed};
use crate::model::stamp;
use sha2::{Digest, Sha256};

pub struct SupersedePairs;
pub struct ForwardOnly;
pub struct Sealed;
pub struct UpdatedMoves;

fn field_of(text: &str, key: &str) -> Option<String> {
    match frontmatter::parse(frontmatter::split(text).raw) {
        Parsed::Map(m) => m.get(key).and_then(|v| v.as_str()).map(str::to_string),
        _ => None,
    }
}

impl Rule for UpdatedMoves {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "T4",
            title: "an edited body moves its updated date",
            explain: "`updated` tells readers how current a document is. spec set, mv and attach keep it current, but a hand edit does not; this reports a commit whose body changed while `updated` stayed. `spec fix --rule T4 --write` sets it to today.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        if ctx.scope.is_all() {
            return;
        }
        let today = stamp::today();
        for &i in &ctx.scope.touched {
            let d = &ctx.corpus.docs[i];
            let (Some(before), Some(now)) = (ctx.previous(&d.path), ctx.current(&d.path)) else { continue };
            let (old_date, new_date) = (field_of(&before, "updated"), field_of(&now, "updated"));
            if body_hash(&before) == body_hash(&now) || old_date != new_date {
                continue;
            }
            let Some(date) = new_date.filter(|u| *u != today) else { continue };
            out.push(
                Finding::warn(
                    "T4",
                    &d.path,
                    key_line(ctx, &d.path, "updated"),
                    format!("the body changed but updated is still {date}"),
                )
                .fix_replace(format!("updated: {date}"), format!("updated: {today}")),
            );
        }
    }
}

impl Rule for SupersedePairs {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "T1",
            title: "superseded documents have exactly one successor",
            explain: "Supersession is written once, on the new document (rel.supersedes). The old document's status must then be superseded, and only one document may claim it. `spec new --from <id>` and `spec set <new> rel.supersedes+=<old>` keep both sides in step.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            let successors: Vec<usize> =
                ctx.graph.edges_to(i).filter(|e| e.kind == "supersedes").map(|e| e.from).collect();
            let covered = ctx.scope.covers(i) || successors.iter().any(|&s| ctx.scope.covers(s));
            if !covered {
                continue;
            }
            let status = d.status().unwrap_or_default();
            if let Some(cycle) = query::supersede_cycle(ctx.graph, i) {
                let ids: Vec<String> = cycle.iter().map(|&s| ctx.corpus.docs[s].id.to_string()).collect();
                out.push(
                    Finding::error(
                        "T1",
                        &d.path,
                        1,
                        format!("supersede cycle, nothing is current: {}", ids.join(" → ")),
                    )
                    .with_candidates(ids),
                );
            }
            if successors.len() > 1 {
                let c = successors.iter().map(|&s| ctx.corpus.docs[s].id.to_string()).collect();
                out.push(
                    Finding::error("T1", &d.path, 1, format!("{} documents supersede {}", successors.len(), d.id))
                        .with_candidates(c),
                );
            }
            if !successors.is_empty() && status != "superseded" && status != "archived" {
                out.push(
                    Finding::error(
                        "T1",
                        &d.path,
                        key_line(ctx, &d.path, "status"),
                        format!("superseded by {} but status is '{status}'", ctx.corpus.docs[successors[0]].id),
                    )
                    .fix_cmd(format!("spec set {} status=superseded", d.id)),
                );
            }
            if successors.is_empty() && status == "superseded" {
                out.push(Finding::error(
                    "T1",
                    &d.path,
                    key_line(ctx, &d.path, "status"),
                    "status is superseded but no document supersedes it",
                ));
            }
        }
    }
}

fn previous_status(ctx: &Ctx, path: &str) -> Option<String> {
    field_of(&ctx.previous(path)?, "status")
}

impl Rule for ForwardOnly {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "T2",
            title: "status only moves forward",
            explain: "draft → active → superseded or archived, and draft → rejected, unless the type sets its own `transitions` in spec.toml. A decision that comes back is a new document that supersedes the old one, so the record of the first decision stays intact.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        if ctx.scope.is_all() {
            return;
        }
        for &i in &ctx.scope.touched {
            let d = &ctx.corpus.docs[i];
            let (Some(before), Some(now)) = (previous_status(ctx, &d.path), d.status()) else { continue };
            let Some(ty) = ctx.corpus.cfg.registry.get(&d.id.ty) else { continue };
            if !ty.transition_allowed(&before, &now) {
                out.push(
                    Finding::error(
                        "T2",
                        &d.path,
                        key_line(ctx, &d.path, "status"),
                        format!("status moved back from '{before}' to '{now}'"),
                    )
                    .fix_cmd(format!("spec new {} --from {} --title '…'", d.id.ty, d.id)),
                );
            }
        }
    }
}

fn body_hash(text: &str) -> String {
    let body = frontmatter::split(text).body;
    Sha256::digest(body.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

impl Rule for Sealed {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "T3",
            title: "sealed documents keep their body",
            explain: "Types marked sealed (audits by default) are snapshots: after the first commit only frontmatter may change. Record a new audit instead of editing an old one.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        if ctx.scope.is_all() {
            return;
        }
        for &i in &ctx.scope.touched {
            let d = &ctx.corpus.docs[i];
            if !ctx.corpus.cfg.registry.get(&d.id.ty).is_some_and(|t| t.sealed) {
                continue;
            }
            let (Some(before), Some(now)) = (ctx.previous(&d.path), ctx.current(&d.path)) else { continue };
            if body_hash(&before) != body_hash(&now) {
                out.push(Finding::error("T3", &d.path, 1, format!("{} is sealed; its body changed", d.id)));
            }
        }
    }
}
