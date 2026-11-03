//! P1: a document lives with the owner of the code it governs.

use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::skipped;
use crate::check::rule::{Rule, RuleInfo};
use crate::model::Tier;
use crate::workspace::placement;

pub struct OwnerMatches;

impl Rule for OwnerMatches {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "P1",
            title: "documents live with the owner of the code they govern",
            explain: "A document belongs in the spec directory of the narrowest owner that contains every path in its rel.governs (a public owner's documents go to its private companion; a retired owner's to the root). Governs inside another project (a remote checked out in the repository, `<remote>:<path>`, or a submodule that no owner kind claims, such as vendored code) are left out of the decision. A submodule an owner kind claims is that owner's code, spec.toml or not, so its documents live in its spec directory (0.5.0 left out every submodule, which sent them to the root). `spec new` starts from the owner containing the directory it runs in (`-C <dir>`). When code moves, or P1 asks, `spec mv <id> --owner <owner> --write` (the fix P1 prints; since 0.5.1 plain `spec mv <id> --write` decides the same owner) moves the document, rewrites every reference to it and carries its `[check] baseline` entries along.",
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
            let all = d.rel().remove("governs").unwrap_or_default();
            if !all.is_empty() && all.iter().all(|g| crate::workspace::projects::is_external(&ctx.corpus.cfg, g)) {
                continue;
            }
            let decision = placement::decide_local(&ctx.corpus.cfg, owners, &all, current);
            if decision.owner.id != current.id {
                out.push(
                    Finding::error(
                        "P1",
                        &d.path,
                        1,
                        format!("belongs to {} ({}), not {}", decision.owner.id, decision.reason, current.id),
                    )
                    .fix_cmd(format!(
                        "spec mv {} --owner {} --write",
                        crate::suggest::shell::word(&d.id),
                        crate::suggest::shell::word(&decision.owner.id)
                    )),
                );
            }
        }
    }
}
