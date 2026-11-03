//! Findings of a full check, shared by `spec check` and the language server.

use crate::cli::args::global::Global;
use crate::session;
use anyhow::Result;
use specfold_core::check::{self, Finding, Options, Range, Selection, baseline};

/// Every finding of a full check (no commands, no network), with the baseline applied.
pub fn all_findings(g: &Global) -> Result<Vec<Finding>> {
    let s = session::open(g)?;
    let commands = crate::cli::describe::commands().into_iter().map(|c| c.name).collect();
    let opts = Options { run_commands: false, external: false, online: false, guards: false, commands };
    let report = check::run(&s.corpus, &s.graph, Range::All, &opts, &Selection { only: vec![], skip: vec![] })?;
    let list = match baseline::load(&s.corpus)? {
        Some(b) => baseline::hold(&s.corpus, &b, report.findings).findings,
        None => report.findings,
    };
    Ok(list)
}
