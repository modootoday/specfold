//! `spec new`.

use super::preview;
use crate::cli::args::{global::Global, write::NewArgs};
use crate::output::Output;
use crate::session;
use anyhow::Result;
use specfold_core::authoring::new::{self, NewDoc};

pub fn run(g: &Global, a: &NewArgs) -> Result<Output> {
    let s = session::open(g)?;
    let governs: Vec<String> = a.governs.iter().map(|p| session::repo_path(&s, p)).collect();
    let req = NewDoc {
        ty: a.ty.clone(),
        title: a.title.clone(),
        governs,
        owner: a.owner.clone(),
        slug: a.slug.clone(),
        from: a.from.clone(),
        local: a.local,
        bundle: a.bundle,
        key: None,
        body: None,
    };
    let (created, plan) = new::plan(&s.corpus, &s.graph, &req)?;
    let o = Output::new("new")
        .unpaged()
        .field("id", created.id.clone())
        .field("path", created.path.clone())
        .field("owner_reason", created.owner_reason.clone())
        .head(format!("{} {}  {}", if a.dry_run { "would create" } else { "created" }, created.id, created.path))
        .head(format!("owner: {}", created.owner_reason))
        .head("dates and timestamp keys are UTC");
    let o = preview::finish(o, &plan, &s.corpus.cfg.root, !a.dry_run, "rerun without --dry-run")?;
    Ok(if a.dry_run { o } else { o.next(format!("spec show {}", created.id)) })
}
