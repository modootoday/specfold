//! `spec fetch`.

use crate::cli::args::{global::Global, read::FetchArgs};
use crate::output::Output;
use crate::session;
use anyhow::Result;
use serde_json::to_value;
use specfold_core::files::fetch;

pub fn run(g: &Global, a: &FetchArgs) -> Result<Output> {
    let s = session::open(g)?;
    let idx = session::resolve(&s, &a.doc)?;
    let got = fetch::fetch(&s.corpus, &s.corpus.docs[idx], a.force)?;
    let mut o = Output::new("fetch");
    for f in &got {
        o = o.item(to_value(f)?, format!("{:<8} {}", f.state, f.path));
    }
    Ok(o.next(format!("spec files {}", specfold_core::shell::word(&s.corpus.docs[idx].id))))
}
