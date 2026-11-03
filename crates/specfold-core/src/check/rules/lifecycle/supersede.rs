//! T1: supersede claims pair up: the older document says what replaced it, and no chain loops.

use crate::check::context::{Ctx, key_line};
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::corpus::graph::query;

pub struct SupersedePairs;

impl Rule for SupersedePairs {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "T1",
            title: "superseded documents have exactly one successor",
            explain: "Supersession is written once, on the new document (rel.supersedes). Once the new document takes effect (active, or the first current status after its type's initial one; a type that starts active takes effect at once), the old document's status must be superseded, and only one document may claim it. A successor still in its initial status, such as a draft, leaves the old document as it is, and so does one that ended without taking effect (rejected). `spec new --from <id>` writes the relation; when the new document starts as a draft, `spec set <new> status=active --write` marks the old one superseded in the same write. `spec set <new> rel.supersedes+=<old> --write` records it by hand.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            // C204: a cross-type supersedes is an L3 error, not a successor.
            let successors: Vec<usize> = ctx
                .graph
                .edges_to(i)
                .filter(|e| e.kind == "supersedes" && ctx.corpus.docs[e.from].id.ty == d.id.ty)
                .map(|e| e.from)
                .collect();
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
            // A successor that has not taken effect (a draft) leaves the old document in force.
            let holding: Vec<usize> = successors.iter().copied().filter(|&s| taken_effect(ctx, s)).collect();
            if !holding.is_empty() && status != "superseded" && status != "archived" {
                out.push(
                    Finding::error(
                        "T1",
                        &d.path,
                        key_line(ctx, &d.path, "status"),
                        format!("superseded by {} but status is '{status}'", ctx.corpus.docs[holding[0]].id),
                    )
                    .fix_cmd(superseded_fix(ctx, d, &status, holding[0])),
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

/// Whether document `i` holds or once held, so what it supersedes must say superseded.
fn taken_effect(ctx: &Ctx, i: usize) -> bool {
    let d = &ctx.corpus.docs[i];
    let status = d.status().unwrap_or_default();
    ctx.corpus.cfg.registry.get(&d.id.ty).is_none_or(|t| t.has_taken_effect(&status))
}

/// C204: the command that marks `d` superseded; through a middle status when the lifecycle
/// has no direct move (draft), else dropping the relation on the successor.
fn superseded_fix(ctx: &Ctx, d: &crate::model::Doc, status: &str, successor: usize) -> String {
    let me = crate::suggest::shell::word(&d.id);
    let Some(ty) = ctx.corpus.cfg.registry.get(&d.id.ty) else {
        return format!("spec set {me} status=superseded --write");
    };
    if ty.transition_allowed(status, "superseded") {
        return format!("spec set {me} status=superseded --write");
    }
    match ty.statuses.iter().find(|m| ty.transition_allowed(status, m) && ty.transition_allowed(m, "superseded")) {
        Some(m) => format!("spec set {me} status={m} --write && spec set {me} status=superseded --write"),
        None => {
            format!(
                "spec set {} rel.supersedes-={} --write",
                crate::suggest::shell::word(&ctx.corpus.docs[successor].id),
                d.id
            )
        }
    }
}
