//! `spec mv`.

use super::preview;
use crate::cli::args::{global::Global, write::MvArgs};
use crate::output::Output;
use crate::session;
use anyhow::Result;
use serde_json::json;
use specfold_core::authoring::mv::{self, Move};
use specfold_core::model::Tier;
use specfold_core::workspace::vcs;

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
    let old_path = s.corpus.docs[idx].path.clone();
    let mut o = o;
    let mut cited = 0;
    if a.cited_paths {
        // Plain-text paths in READMEs and notes are not references, so mv does not rewrite
        // them; listing them lets the caller fix each one.
        let root = &s.corpus.cfg.root;
        let docs: std::collections::HashSet<&str> = s.corpus.docs.iter().map(|d| d.path.as_str()).collect();
        for f in vcs::committed(root).unwrap_or_default() {
            if !f.ends_with(".md") || docs.contains(f.as_str()) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(root.join(&f)) else { continue };
            for (i, line) in text.lines().enumerate() {
                if line.contains(&old_path) {
                    cited += 1;
                    let at = format!("{f}:{}", i + 1);
                    o = o.item(
                        json!({"path": at, "cites": old_path, "now": moved.path}),
                        format!("cites old path  {at}"),
                    );
                }
            }
        }
        o = o.field("cited_paths", cited);
    }
    let o = preview::finish(o, &plan, &s.corpus.cfg.root, a.write, &format!("spec mv {} --write", a.doc))?;
    Ok(if cited > 0 && a.write {
        o.next(format!("replace {old_path} with {} in the {cited} line(s) listed", moved.path))
    } else if cited > 0 {
        o.foot(format!("{cited} line(s) outside the documents cite the old path; fix them after the move"))
    } else if a.write {
        o.next(format!("spec check --rule L1 --rule P1 --rule S2  # then: spec show {}", moved.to))
    } else {
        o
    })
}
