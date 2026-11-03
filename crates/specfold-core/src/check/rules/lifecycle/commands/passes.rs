//! X1: a source of truth's verify commands pass.

use super::job::{report, settings, timeout, trace_slowest};
use super::select::{selected, verify_commands};
use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::plugins::runner::{self, Job};

pub struct VerifyPasses;

impl Rule for VerifyPasses {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "X1",
            title: "a source of truth's verify commands pass",
            explain: "A sot lists in `verify` the commands that prove the code obeys it. They run for every sot whose governed code changed, with a deadline ([verify] timeout_ms, or verify_timeout_ms on the document), in parallel and each distinct command once. A command whose program is not installed is reported as unable to run rather than passing or failing, because a check that cannot run proves nothing.",
            on_commit: false,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        if !ctx.opts.run_commands {
            return;
        }
        let start = std::time::Instant::now();
        let mut jobs = vec![];
        let mut owners = vec![];
        let picked = selected(ctx);
        for &i in &picked {
            let d = &ctx.corpus.docs[i];
            if d.status().as_deref() != Some("active") {
                continue;
            }
            for cmd in verify_commands(d) {
                jobs.push(Job { cmd: cmd.clone(), env: vec![], timeout: timeout(ctx, d) });
                owners.push((i, cmd));
            }
        }
        let start =
            crate::corpus::trace::phase(&format!("X1 select ({} docs, {} jobs)", picked.len(), jobs.len()), start);
        let outcomes = runner::run(&ctx.corpus.cfg.spec_home, &jobs, &settings(ctx));
        crate::corpus::trace::phase("X1 commands", start);
        trace_slowest(&jobs, &outcomes);
        for ((i, cmd), o) in owners.iter().zip(&outcomes) {
            out.extend(report("X1", ctx, &ctx.corpus.docs[*i], "verify", cmd, o));
        }
    }
}
