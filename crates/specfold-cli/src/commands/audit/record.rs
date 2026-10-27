//! Recording an audit as a sealed `audit` document.

use crate::cli::args::verify::AuditArgs;
use crate::output::Output;
use crate::session::Session;
use anyhow::Result;
use specfold_core::authoring::new::{self, NewDoc};
use specfold_core::model::stamp;

pub fn run(s: &Session, a: &AuditArgs, body: String) -> Result<Output> {
    let title = a.title.clone().unwrap_or_else(|| format!("Audit {}", stamp::today()));
    let req = NewDoc { ty: "audit".into(), title, body: Some(body), ..Default::default() };
    let (created, plan) = new::plan(&s.corpus, &s.graph, &req)?;
    let o = Output::new("audit").unpaged().field("id", created.id.clone()).field("path", created.path.clone());
    if !a.write {
        return Ok(o.head(format!("would record {}  {}", created.id, created.path)).next("add --write to record"));
    }
    plan.apply(&s.corpus.cfg.root)?;
    Ok(o.head(format!("recorded {}  {}", created.id, created.path)).next(format!("spec show {}", created.id)))
}
