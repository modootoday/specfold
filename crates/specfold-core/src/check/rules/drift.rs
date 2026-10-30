//! D1: in a staged or ranged run, code changed under an active document's `governs` while the
//! document, and every change note pointing at it, stayed as they were.

use super::support::skipped;
use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::graph::query::governing;
use crate::model::Tier;
use std::collections::{BTreeMap, BTreeSet};

pub struct GovernedCodeDrift;

/// How many changed paths a finding names before it says "+N more".
const SHOWN: usize = 5;

impl Rule for GovernedCodeDrift {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "D1",
            title: "changed code moves its governing document",
            explain: "In a staged or ranged run (`spec check --staged`, `--changed`), each changed file that is not a document is matched against the explicit `rel.governs` globs of active, committed documents. A document whose governed code changed is expected to change in the same range, or to be named by a `change` note that changes in the range; otherwise this warning lists the changed paths. Documents that govern only through their owner directory are not asked, and a full run never reports D1. Update the document (or its `verified_at`), or write a change note: spec new change --title … and link the document.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        if ctx.scope.is_all() {
            return;
        }
        let c = ctx.corpus;
        let own: BTreeSet<&String> =
            c.docs.iter().flat_map(|d| std::iter::once(&d.path).chain(&d.chapters).chain(&d.bundle_files)).collect();
        let mut drift: BTreeMap<usize, Vec<&str>> = BTreeMap::new();
        for p in ctx.scope.paths.iter().filter(|p| !own.contains(p)) {
            for g in governing(c, p) {
                let d = &c.docs[g.idx];
                let explicit = d.rel().get("governs").is_some_and(|v| !v.is_empty());
                if explicit && d.tier == Tier::Committed && d.status().as_deref() == Some("active") && !skipped(d, "D1")
                {
                    drift.entry(g.idx).or_default().push(p);
                }
            }
        }
        for (idx, paths) in drift {
            let moved = ctx.scope.touched.contains(&idx)
                || ctx
                    .graph
                    .edges_to(idx)
                    .any(|e| ctx.scope.touched.contains(&e.from) && c.docs[e.from].id.ty == "change");
            if moved {
                continue;
            }
            let d = &c.docs[idx];
            let mut named = paths.iter().take(SHOWN).copied().collect::<Vec<_>>().join(", ");
            if paths.len() > SHOWN {
                named.push_str(&format!(" (+{} more)", paths.len() - SHOWN));
            }
            out.push(
                Finding::warn(
                    "D1",
                    &d.path,
                    1,
                    format!("{named} changed under {}'s governs, but the document and its change notes did not", d.id),
                )
                .fix_cmd(format!("spec show {}", d.id)),
            );
        }
    }
}
