//! `spec check`.

use crate::cli::args::{global::Global, verify::CheckArgs};
use crate::output::{Code, Output, findings};
use crate::session;
use anyhow::Result;
use specfold_core::check::{self, Options, Range, Selection};
use std::time::Instant;

pub fn run(g: &Global, a: &CheckArgs) -> Result<Output> {
    let started = Instant::now();
    let s = session::open(g)?;
    let range = match (&a.changed, a.staged) {
        (Some(base), _) => Range::Changed(base.clone()),
        (None, true) => Range::Staged,
        (None, false) => Range::All,
    };
    let opts = Options { run_commands: a.run_commands, external: a.external, online: a.online };
    let report =
        check::run(&s.corpus, &s.graph, range, &opts, &Selection { only: a.rules.clone(), skip: a.skip.clone() })?;
    let (errors, warnings) = findings::counts(&report.findings);
    let ms = started.elapsed().as_millis();
    let o = Output::new("check")
        .field("errors", errors)
        .field("warnings", warnings)
        .field("docs_checked", report.docs_checked)
        .field("docs_total", s.corpus.docs.len())
        .field("rules", report.rules_run.clone())
        .field("elapsed_ms", ms as u64);
    let o = findings::push_all(o, &report.findings).foot(format!(
        "{errors} error(s), {warnings} warning(s) in {} of {} documents · {:.2}s",
        report.docs_checked,
        s.corpus.docs.len(),
        ms as f64 / 1000.0
    ));
    let has_fix = report.findings.iter().any(|f| f.fix.is_some());
    Ok(match (errors, has_fix) {
        (0, _) => o.next("spec audit"),
        (_, true) => {
            o.code(Code::Findings).next(if a.staged { "spec fix --staged --write" } else { "spec fix --write" })
        }
        _ => o.code(Code::Findings).next("spec explain <rule>"),
    })
}
