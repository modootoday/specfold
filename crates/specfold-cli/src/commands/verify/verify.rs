//! `spec verify`: run the verify commands of active sources of truth.

use crate::cli::args::{global::Global, verify::VerifyArgs};
use crate::output::{Code, Output, findings};
use crate::session;
use anyhow::Result;
use specfold_core::check::{self, Options, Range, Selection};

pub fn run(g: &Global, a: &VerifyArgs) -> Result<Output> {
    let s = session::open(g)?;
    let range = match &a.changed {
        Some(base) => Range::Changed(base.clone()),
        None => Range::All,
    };
    let sel = Selection { only: vec!["X1".into(), "G1".into()], skip: vec![] };
    let report = check::run(&s.corpus, &s.graph, range, &Options { run_commands: true, ..Default::default() }, &sel)?;
    let only = match &a.doc {
        Some(r) => Some(s.corpus.docs[session::resolve(&s, r)?].path.clone()),
        None => None,
    };
    let list: Vec<_> = report.findings.into_iter().filter(|f| only.as_ref().is_none_or(|p| &f.path == p)).collect();
    let (errors, _) = findings::counts(&list);
    let o = findings::push_all(Output::new("verify").field("failed", errors), &list);
    Ok(if errors == 0 {
        o.foot("every verify command passed")
    } else {
        // C129: name the first failing document rather than a placeholder.
        let first = list.iter().find(|f| f.severity == specfold_core::check::Severity::Error).map(|f| f.path.clone());
        let id = first
            .and_then(|p| s.corpus.docs.iter().find(|d| d.path == p))
            .map(|d| d.id.to_string())
            .unwrap_or_default();
        o.code(Code::Findings).next(format!("spec show {} --section verification", specfold_core::shell::quote(&id)))
    })
}
