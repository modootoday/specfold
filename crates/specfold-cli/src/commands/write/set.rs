//! `spec set`.

use super::preview;
use crate::cli::args::{global::Global, write::SetArgs};
use crate::output::Output;
use crate::session;
use anyhow::{Result, anyhow, bail};
use specfold_core::authoring::set::{self, Assign, Op};
use specfold_core::translation;

pub fn run(g: &Global, a: &SetArgs) -> Result<Output> {
    let s = session::open(g)?;
    let idx = session::resolve(&s, &a.doc)?;
    let mut assigns = a.assignments.iter().map(|x| Assign::parse(x)).collect::<Result<Vec<_>>>()?;
    let mut shown = a.assignments.clone();
    if a.source_rev {
        let d = &s.corpus.docs[idx];
        let Some(src) = translation::source_of(&s.corpus, &s.graph, idx) else {
            bail!(
                "{} has no rel.translation_of that names a document; add it first: spec set {} rel.translation_of+=<source id>",
                d.id,
                d.id
            );
        };
        let rev =
            translation::rev_of(&s.corpus, src).ok_or_else(|| anyhow!("cannot read {}", s.corpus.docs[src].path))?;
        shown.push(format!("{}={rev}", translation::KEY));
        assigns.push(Assign { key: translation::KEY.into(), op: Op::Set, value: rev });
    }
    for x in assigns.iter().filter(|x| matches!(x.op, Op::Set)) {
        let problem = match x.key.as_str() {
            "lang" => translation::lang_problem(&x.value),
            translation::KEY => translation::rev_problem(&x.value).map(|p| format!("{p}; record it with --source-rev")),
            _ => None,
        };
        if let Some(p) = problem {
            bail!("{p}");
        }
    }
    let plan = set::plan(&s.corpus, idx, &assigns)?;
    let id = s.corpus.docs[idx].id.to_string();
    let ty = s.corpus.docs[idx].id.ty.clone();
    let unknown: Vec<String> =
        assigns.iter().filter_map(|x| specfold_core::check::rules::unknown_key(&s.corpus, &ty, &x.key)).collect();
    let o = Output::new("set").unpaged().field("id", id.clone()).head(format!("{id}: {}", shown.join(" ")));
    let o = preview::finish(o, &plan, &s.corpus.cfg.root, !a.dry_run, "rerun without --dry-run")?;
    let o = unknown.iter().fold(o.field("warnings", unknown.clone()), |o, w| o.foot(format!("warn  S1  {w}")));
    Ok(if a.dry_run {
        o
    } else {
        o.next(format!("spec check --rule S1 --rule T1 --rule S4 --rule L3  # then: spec show {id}"))
    })
}
