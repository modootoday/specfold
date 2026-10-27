//! X1, X2 and G1: the commands a document names actually pass, and an active source of truth
//! says how it is verified. X1 and G1 only run when asked (`spec verify`,
//! `spec check --run-commands`), because they execute project code.

mod select;

use super::support::key_line;
use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rule::{Rule, RuleInfo};
use crate::model::Doc;
use crate::runner::{self, Job, Outcome, Settings, Verdict};
use regex::Regex;
use select::{selected, verify_commands};
use std::time::Duration;

pub struct VerifyPasses;
pub struct VerifyDeclared;
pub struct GeneratedInSync;

fn settings(ctx: &Ctx) -> Settings {
    let v = &ctx.corpus.cfg.raw.verify;
    Settings {
        jobs: v.jobs,
        serial: v.serial.iter().filter_map(|r| Regex::new(r).ok()).collect(),
        shell: v.shell.clone(),
    }
}

fn timeout(ctx: &Ctx, d: &Doc) -> Duration {
    let own = d.fm().and_then(|m| m.get("verify_timeout_ms")).and_then(|v| v.as_u64());
    Duration::from_millis(own.unwrap_or(ctx.corpus.cfg.raw.verify.timeout_ms))
}

fn report(rule: &'static str, ctx: &Ctx, d: &Doc, key: &str, cmd: &str, o: &Outcome) -> Option<Finding> {
    let line = key_line(ctx, &d.path, key);
    let detail = |s: &str| if s.is_empty() { String::new() } else { format!(": {s}") };
    let msg = match &o.verdict {
        Verdict::Pass => return None,
        Verdict::Fail(code) => {
            let code = code.map(|c| c.to_string()).unwrap_or_else(|| "signal".into());
            format!("{key} failed (exit {code}){}: {cmd}", detail(&o.tail))
        }
        Verdict::Timeout => format!("{key} timed out after {} ms: {cmd}", o.elapsed.as_millis()),
        Verdict::CannotRun(why) => format!("{key} cannot run ({why}): {cmd}"),
    };
    Some(Finding::error(rule, &d.path, line, msg))
}

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
        let mut jobs = vec![];
        let mut owners = vec![];
        for i in selected(ctx) {
            let d = &ctx.corpus.docs[i];
            if d.status().as_deref() != Some("active") {
                continue;
            }
            for cmd in verify_commands(d) {
                jobs.push(Job { cmd: cmd.clone(), env: vec![], timeout: timeout(ctx, d) });
                owners.push((i, cmd));
            }
        }
        let outcomes = runner::run(&ctx.corpus.cfg.root, &jobs, &settings(ctx));
        for ((i, cmd), o) in owners.iter().zip(&outcomes) {
            out.extend(report("X1", ctx, &ctx.corpus.docs[*i], "verify", cmd, o));
        }
    }
}

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
                        .fix_cmd(format!("spec set {} verify=manual", d.id)),
                );
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
