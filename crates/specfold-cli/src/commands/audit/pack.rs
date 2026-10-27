//! `spec audit --pack`: documents to review together for contradictions and stale statements.

use crate::cli::args::verify::AuditArgs;
use crate::output::Output;
use crate::session::Session;
use anyhow::Result;
use serde_json::to_value;
use specfold_core::audit::pack;

pub fn run(s: &Session, a: &AuditArgs) -> Result<Output> {
    let packs = pack::build(&s.corpus, &s.graph, &a.owners, a.budget);
    let mut o = Output::new("audit").field("mode", "pack").head(format!(
        "{} review pack(s), each under {} bytes",
        packs.len(),
        a.budget
    ));
    for p in &packs {
        o = o.item(
            to_value(p)?,
            format!("{}  ({} docs, {} bytes)\n    {}", p.topic, p.docs.len(), p.bytes, p.docs.join(" ")),
        );
    }
    Ok(o.next("review each pack, then: spec audit record --from review.md --write"))
}
