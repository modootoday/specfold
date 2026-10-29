//! `spec mv`.

use super::preview;
use crate::cli::args::{global::Global, write::MvArgs};
use crate::output::Output;
use crate::session;
use anyhow::Result;
use specfold_core::authoring::mv::{self, Move};
use specfold_core::model::Tier;

pub fn run(g: &Global, a: &MvArgs) -> Result<Output> {
    let s = session::open(g)?;
    let idx = session::resolve(&s, &a.doc)?;
    let (moved, plan) = mv::plan(
        &s.corpus,
        idx,
        &Move {
            owner: a.owner.clone(),
            slug: a.slug.clone(),
            tier: a.tier.as_deref().map(|t| if t == "local" { Tier::Local } else { Tier::Committed }),
        },
    )?;
    let o = Output::new("mv")
        .unpaged()
        .field("from", moved.from.clone())
        .field("to", moved.to.clone())
        .field("path", moved.path.clone())
        .field("references", moved.references)
        .head(format!("{} -> {}  ({} referring document(s) rewritten)", moved.from, moved.to, moved.references));
    let o = preview::finish(o, &plan, &s.corpus.cfg.root, a.write, &format!("spec mv {} --write", a.doc))?;
    Ok(if a.write {
        o.next(format!("spec check --rule L1 --rule P1 --rule S2  # then: spec show {}", moved.to))
    } else {
        o
    })
}
