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
        o.code(Code::Findings).next("spec show <id> --section verification")
    })
}
