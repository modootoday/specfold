//! Running a document's commands: the runner settings, each command's deadline, and a finding
//! for a command that did not pass.

use crate::check::context::Ctx;
use crate::check::finding::Finding;
use crate::check::rules::support::key_line;
use crate::model::Doc;
use crate::runner::{Job, Outcome, Settings, Verdict};
use regex::Regex;
use std::time::Duration;

pub(super) fn settings(ctx: &Ctx) -> Settings {
    let v = &ctx.corpus.cfg.raw.verify;
    Settings {
        jobs: v.jobs,
        serial: v.serial.iter().filter_map(|r| Regex::new(r).ok()).collect(),
        shell: v.shell.clone(),
    }
}

pub(super) fn timeout(ctx: &Ctx, d: &Doc) -> Duration {
    let own = d.fm().and_then(|m| m.get("verify_timeout_ms")).and_then(|v| v.as_u64());
    Duration::from_millis(own.unwrap_or(ctx.corpus.cfg.raw.verify.timeout_ms))
}

pub(super) fn report(rule: &'static str, ctx: &Ctx, d: &Doc, key: &str, cmd: &str, o: &Outcome) -> Option<Finding> {
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

/// The slowest commands, for `SPECFOLD_TRACE`.
pub(super) fn trace_slowest(jobs: &[Job], outcomes: &[Outcome]) {
    if !crate::corpus::trace::enabled() {
        return;
    }
    let mut seen = std::collections::BTreeMap::new();
    for (j, o) in jobs.iter().zip(outcomes) {
        seen.entry(j.cmd.as_str()).or_insert(o.elapsed);
    }
    let mut v: Vec<_> = seen.into_iter().collect();
    v.sort_by_key(|x| std::cmp::Reverse(x.1));
    for (cmd, t) in v.into_iter().take(5) {
        eprintln!("specfold:   X1 {:>9.1} ms  {cmd}", t.as_secs_f64() * 1000.0);
    }
}
