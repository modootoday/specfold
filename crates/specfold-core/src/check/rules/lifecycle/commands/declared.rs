//! X2: an active source of truth says how it is verified.

use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};

pub struct VerifyDeclared;

impl Rule for VerifyDeclared {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "X2",
            title: "an active source of truth says how it is verified",
            explain: "A type that has a `verify` field expects its active documents to name the commands that prove them, or to say `verify: manual` when people check them by hand. A rule with neither cannot tell anyone whether it still holds.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if !ctx.scope.covers(i) || d.status().as_deref() != Some("active") {
                continue;
            }
            let expects = ctx.corpus.cfg.registry.get(&d.id.ty).is_some_and(|t| t.fields.iter().any(|f| f == "verify"));
            if expects && d.list_field("verify").is_empty() {
                out.push(
                    Finding::warn("X2", &d.path, 1, format!("{} is active but declares no verify command", d.id))
                        .fix_cmd(format!("spec set {} verify=manual --write", crate::shell::word(&d.id))),
                );
            }
        }
    }
}
