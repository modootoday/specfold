//! C3: `spec check --commits <sha>...`, each commit checked on its own. A commit's scope is the
//! files it changed; their text is read as it is now, so a later fix already counts.

use crate::cli::args::{global::Global, verify::CheckArgs};
use crate::output::{Code, Output, findings};
use crate::session;
use anyhow::{Result, bail};
use specfold_core::check::{self, Options, Range, Selection, baseline};
use specfold_core::workspace::vcs;

pub fn run(g: &Global, a: &CheckArgs) -> Result<Output> {
    let s = session::open(g)?;
    let root = &s.corpus.cfg.spec_home;
    let commands = crate::cli::describe::commands().into_iter().map(|c| c.name).collect();
    let opts = Options { run_commands: false, external: a.external, online: false, guards: false, commands };
    let sel = Selection { only: a.rules.clone(), skip: a.skip.clone() };
    let base = if a.no_baseline { None } else { baseline::load(&s.corpus)? };
    let mut o = Output::new("check");
    let (mut total_new, mut total_held, mut rows) = (0, 0, vec![]);
    for sha in &a.commits {
        let Some(full) = vcs::resolve(root, sha) else {
            bail!("--commits {sha}: no such commit; next: git log --oneline");
        };
        let short: String = full.chars().take(12).collect();
        let report = check::run(&s.corpus, &s.graph, Range::Commit(full.clone()), &opts, &sel)?;
        let (list, held) = match &base {
            Some(b) => {
                let h = baseline::hold(&s.corpus, b, report.findings);
                (h.findings, h.known)
            }
            None => (report.findings, 0),
        };
        let (errors, warnings) = findings::counts(&list);
        total_new += errors;
        total_held += held;
        rows.push(serde_json::json!({"commit": full, "errors": errors, "warnings": warnings, "held": held}));
        o = o.item(
            serde_json::json!({"commit": full, "errors": errors, "warnings": warnings, "held": held}),
            format!("commit {short}: {errors} new error(s), {warnings} warning(s), {held} held by the baseline"),
        );
        o = findings::push_all(o, &list);
    }
    let o = o
        .field("commits", rows)
        .field("errors", total_new)
        .field("held", total_held)
        .foot(format!("{} commit(s): {total_new} new error(s), {total_held} held by the baseline", a.commits.len()));
    Ok(if total_new > 0 {
        o.code(Code::Findings).next("spec check --changed <base>  # the whole range at once")
    } else {
        o.next("spec audit  # optional: stale, orphan and unfinished documents")
    })
}
