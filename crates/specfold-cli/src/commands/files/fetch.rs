//! `spec fetch`.

use crate::cli::args::{global::Global, read::RefArgs};
use crate::output::Output;
use crate::session;
use anyhow::Result;
use serde_json::to_value;
use specfold_core::files::fetch;

pub fn run(g: &Global, a: &RefArgs) -> Result<Output> {
    let s = session::open(g)?;
    let idx = session::resolve(&s, &a.doc)?;
    let got = fetch::fetch(&s.corpus, &s.corpus.docs[idx])?;
    let mut o = Output::new("fetch");
    for f in &got {
        o = o.item(to_value(f)?, format!("{:<8} {}", f.state, f.path));
    }
    Ok(o.next(format!("spec files {}", s.corpus.docs[idx].id)))
}
