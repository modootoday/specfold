//! `spec set`.

use super::preview;
use crate::cli::args::{global::Global, write::SetArgs};
use crate::output::Output;
use crate::session;
use anyhow::Result;
use specfold_core::authoring::set::{self, Assign};

pub fn run(g: &Global, a: &SetArgs) -> Result<Output> {
    let s = session::open(g)?;
    let idx = session::resolve(&s, &a.doc)?;
    let assigns = a.assignments.iter().map(|x| Assign::parse(x)).collect::<Result<Vec<_>>>()?;
    let plan = set::plan(&s.corpus, idx, &assigns)?;
    let id = s.corpus.docs[idx].id.to_string();
    let o = Output::new("set").unpaged().field("id", id.clone()).head(format!("{id}: {}", a.assignments.join(" ")));
    let o = preview::finish(o, &plan, &s.corpus.cfg.root, !a.dry_run, "rerun without --dry-run")?;
    Ok(if a.dry_run { o } else { o.next(format!("spec check --rule T1 --rule S4 --rule L3  # then: spec show {id}")) })
}
