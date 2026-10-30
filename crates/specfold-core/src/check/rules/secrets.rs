//! U2: documents hold no secret in a built-in format. `spec export` refuses the same hits, so
//! the commit hook catches them before an export or a push does.

use super::support::skipped;
use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::guards;

pub struct NoSecrets;

impl Rule for NoSecrets {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "U2",
            title: "documents hold no secrets",
            explain: "Every document file (the main file and its chapters) is scanned with the built-in secret formats that `spec export` always applies: private keys, cloud and token formats, and URLs with a password. No guard needs to be configured. The value is never printed. When a configured guard with secrets = true already runs as U1 in the same check, U2 stays quiet so a hit is reported once. Move the value to the environment or a secret store and write a placeholder instead; `spec guard secrets` runs the same check over any range.",
            on_commit: true,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        let u1_runs = !(ctx.scope.is_all() && !ctx.opts.guards);
        if u1_runs && ctx.corpus.cfg.raw.guards.iter().any(|g| g.secrets) {
            return;
        }
        for (i, d) in ctx.corpus.docs.iter().enumerate() {
            if !ctx.scope.covers(i) || skipped(d, "U2") {
                continue;
            }
            let blobs: Vec<(String, Vec<u8>)> = std::iter::once(&d.path)
                .chain(&d.chapters)
                .filter_map(|p| Some((p.clone(), ctx.current(p)?.into_bytes())))
                .collect();
            for (_, mut f) in guards::builtin_secrets(&blobs) {
                f.rule = "U2".into();
                out.push(f);
            }
        }
    }
}
