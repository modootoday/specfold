//! `spec attach`.

use crate::cli::args::{global::Global, write::AttachArgs};
use crate::output::Output;
use crate::session;
use anyhow::Result;
use serde_json::to_value;
use specfold_core::files::attach;

pub fn run(g: &Global, a: &AttachArgs) -> Result<Output> {
    let s = session::open(g)?;
    let idx = session::resolve(&s, &a.doc)?;
    let d = &s.corpus.docs[idx];
    let place = if a.store {
        attach::Where::Store
    } else if a.repo {
        attach::Where::Repo
    } else {
        attach::Where::Auto
    };
    let plan = attach::plan(&s.corpus, d, &a.files, &a.into, place)?;
    let mut o = Output::new("attach").field("id", d.id.to_string());
    if let Some((from, to)) = &plan.to_bundle {
        o = o.head(format!("{from} becomes a bundle: {to}"));
    }
    for f in &plan.files {
        o = o.item(to_value(f)?, format!("{:<5} {:>10}  {}  <- {}", f.store, f.bytes, f.path, f.source));
    }
    if a.write {
        attach::apply(&s.corpus, d, &plan)?;
        return Ok(o.foot("attached").next(format!("spec files {}", d.id)));
    }
    let files: Vec<String> = a.files.iter().map(|p| p.display().to_string()).collect();
    Ok(o.foot("preview only; nothing was written").next(format!("spec attach {} {} --write", a.doc, files.join(" "))))
}
