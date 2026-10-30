//! P1: a document lives with the owner of the code it governs.

use super::support::skipped;
use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::model::Tier;
use crate::placement;

pub struct OwnerMatches;

impl Rule for OwnerMatches {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "P1",
            title: "documents live with the owner of the code they govern",
            explain: "A document belongs in the spec directory of the narrowest owner that contains every path in its rel.governs (a public owner's documents go to its private companion; a retired owner's to the root). When code moves, `spec mv <id>` moves the document and rewrites every reference to it.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        let owners = &ctx.corpus.owners;
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if !ctx.scope.covers(i) || d.tier == Tier::Local || skipped(d, "P1") {
                continue;
            }
            let Some(current) = owners.by_id(&d.id.owner) else { continue };
            let governs = d.rel().remove("governs").unwrap_or_default();
            let decision = placement::decide(owners, &governs, current);
            if decision.owner.id != current.id {
                out.push(
                    Finding::error(
                        "P1",
                        &d.path,
                        1,
                        format!("belongs to {} ({}), not {}", decision.owner.id, decision.reason, current.id),
                    )
                    .fix_cmd(format!("spec mv {} --write", crate::shell::word(&d.id))),
                );
            }
        }
    }
}
