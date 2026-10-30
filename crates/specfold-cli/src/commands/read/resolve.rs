//! `spec resolve`.

use crate::cli::args::{global::Global, read::RefArgs};
use crate::output::Output;
use crate::session;
use anyhow::Result;

pub fn run(g: &Global, a: &RefArgs) -> Result<Output> {
    let (s, doc, remote) = session::open_for(g, &a.doc)?;
    let idx = session::resolve(&s, &doc)?;
    let d = &s.corpus.docs[idx];
    let shown = remote.as_ref().map_or(d.id.to_string(), |r| format!("{r}:{}", d.id));
    Ok(Output::new("resolve")
        .unpaged()
        .field("remote", remote)
        .field("id", d.id.to_string())
        .field("path", d.path.clone())
        .field("status", d.status())
        .head(format!("{shown}  {}", d.path))
        .next(format!("spec show {}", specfold_core::shell::word(&shown))))
}
