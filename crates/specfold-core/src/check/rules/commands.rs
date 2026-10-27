//! X1 and G1: the commands a document names actually pass. Both only run when asked
//! (`spec verify`, `spec check --run-commands`), because they execute project code.

use super::support::key_line;
use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::graph::query;
use std::process::Command;

pub struct VerifyPasses;
pub struct GeneratedInSync;

fn shell(root: &std::path::Path, cmd: &str, check_mode: bool) -> (bool, String) {
    let (prog, flag) = if cfg!(windows) { ("cmd", "/C") } else { ("sh", "-c") };
    let mut c = Command::new(prog);
    c.arg(flag).arg(cmd).current_dir(root);
    if check_mode {
        c.env("SPECFOLD_CHECK", "1");
    }
    match c.output() {
        Ok(o) => {
            let tail: String = String::from_utf8_lossy(&o.stderr)
                .lines()
                .chain(String::from_utf8_lossy(&o.stdout).lines())
                .last()
                .unwrap_or("")
                .chars()
                .take(200)
                .collect();
            (o.status.success(), tail)
        }
        Err(e) => (false, e.to_string()),
    }
}

/// Documents whose governed code changed in this run, or every document for a full run.
fn selected(ctx: &Ctx) -> Vec<usize> {
    if ctx.scope.is_all() {
        return (0..ctx.corpus.docs.len()).collect();
    }
    let mut out: Vec<usize> =
        ctx.scope.paths.iter().flat_map(|p| query::governing(ctx.corpus, p).into_iter().map(|g| g.idx)).collect();
    out.extend(ctx.scope.touched.iter().copied());
    out.sort();
    out.dedup();
    out
}

impl Rule for VerifyPasses {
    fn info(&self) -> &RuleInfo {
        &RuleInfo {
            id: "X1",
            title: "a source of truth's verify commands pass",
            explain: "A sot lists in `verify` the commands that prove the code obeys it. They run for every sot whose governed code changed, so a rule cannot silently stop holding.",
            on_commit: false,
        }
    }

    fn run(&self, ctx: &Ctx, out: &mut Vec<Finding>) {
        if !ctx.opts.run_commands {
            return;
        }
        for i in selected(ctx) {
            let d = &ctx.corpus.docs[i];
            if d.status().as_deref() != Some("active") {
                continue;
            }
            for cmd in d.list_field("verify") {
                let (ok, tail) = shell(&ctx.corpus.cfg.root, &cmd, false);
                if !ok {
                    out.push(Finding::error(
                        "X1",
                        &d.path,
                        key_line(ctx, &d.path, "verify"),
                        format!("verify failed: {cmd} ({tail})"),
                    ));
                }
            }
        }
    }
}

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
        for i in selected(ctx) {
            let d = &ctx.corpus.docs[i];
            let Some(cmd) = d.str_field("generated_by") else { continue };
            let (ok, tail) = shell(&ctx.corpus.cfg.root, &cmd, true);
            if !ok {
                out.push(
                    Finding::error(
                        "G1",
                        &d.path,
                        key_line(ctx, &d.path, "generated_by"),
                        format!("out of date with its generator ({tail})"),
                    )
                    .fix_cmd(cmd),
                );
            }
        }
    }
}
