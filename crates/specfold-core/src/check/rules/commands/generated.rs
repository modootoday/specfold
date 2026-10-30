//! G1: documents produced from code match their generator.

use super::select::selected;
use super::{GeneratedInSync, report, settings, timeout};
use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::runner::{self, Job};

impl Rule for GeneratedInSync {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "G1",
            title: "generated documents match their generator",
            explain: "A document produced from code names its generator in `generated_by`. That command is run with SPECFOLD_CHECK=1 and must exit 0 when the document is current; hand edits are caught here.",
            on_commit: false,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        if !ctx.opts.run_commands {
            return;
        }
        let env = vec![("SPECFOLD_CHECK".to_string(), "1".to_string())];
        let mut jobs = vec![];
        let mut owners = vec![];
        for i in selected(ctx) {
            let d = &ctx.corpus.docs[i];
            let Some(cmd) = d.str_field("generated_by") else { continue };
            jobs.push(Job { cmd: cmd.clone(), env: env.clone(), timeout: timeout(ctx, d) });
            owners.push((i, cmd));
        }
        let outcomes = runner::run(&ctx.corpus.cfg.root, &jobs, &settings(ctx));
        for ((i, cmd), o) in owners.iter().zip(&outcomes) {
            let d = &ctx.corpus.docs[*i];
            out.extend(report("G1", ctx, d, "generated_by", cmd, o).map(|f| f.fix_cmd(cmd.clone())));
        }
    }
}
