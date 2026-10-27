//! `spec resolve`.

use crate::cli::args::{global::Global, read::RefArgs};
use crate::output::Output;
use crate::session;
use anyhow::Result;

pub fn run(g: &Global, a: &RefArgs) -> Result<Output> {
    let s = session::open(g)?;
    let idx = session::resolve(&s, &a.doc)?;
    let d = &s.corpus.docs[idx];
    Ok(Output::new("resolve")
        .unpaged()
        .field("id", d.id.to_string())
        .field("path", d.path.clone())
        .field("status", d.status())
        .head(format!("{}  {}", d.id, d.path))
        .next(format!("spec show {}", d.id)))
}
