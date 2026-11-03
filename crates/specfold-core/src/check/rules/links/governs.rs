//! L5: governed paths exist.

use crate::check::context::{Ctx, key_line};
use crate::check::finding::Finding;
use crate::check::rule::skipped;
use crate::check::rule::{Rule, RuleInfo};

pub struct GovernsExists;

impl Rule for GovernsExists {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "L5",
            title: "governed paths exist",
            explain: "rel.governs names the code a document rules. When that code moves or is deleted the document is orphaned; update governs, move the document with `spec mv`, or archive it.",
            on_commit: false,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if !ctx.scope.covers(i) || skipped(d, "L5") {
                continue;
            }
            for g in d.rel().remove("governs").unwrap_or_default() {
                if crate::workspace::projects::is_external(&ctx.corpus.cfg, &g) {
                    continue;
                }
                let base = g.split(['*', '?', '[', '{']).next().unwrap_or("").trim_end_matches('/');
                let base = if g.contains(['*', '?', '[', '{']) {
                    base.rsplit_once('/').map(|x| x.0).unwrap_or(base)
                } else {
                    base
                };
                if !base.is_empty() && !ctx.corpus.cfg.spec_home.join(base).exists() {
                    out.push(Finding::warn(
                        "L5",
                        &d.path,
                        key_line(ctx, &d.path, "rel"),
                        format!("governs '{g}' but {base} does not exist"),
                    ));
                }
            }
        }
    }
}
