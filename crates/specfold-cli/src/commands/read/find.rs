//! `spec find`.

use crate::cli::args::{global::Global, read::FindArgs};
use crate::output::Output;
use crate::session;
use anyhow::Result;
use serde_json::to_value;
use specfold_core::markdown::{frontmatter, tasks};
use specfold_core::search::{self, Query};

/// Unchecked task items in a document and its chapters.
fn open_tasks(root: &std::path::Path, files: &[&str]) -> usize {
    files
        .iter()
        .map(|f| {
            let text = std::fs::read_to_string(root.join(f)).unwrap_or_default();
            tasks::list(frontmatter::split(&text).body).iter().filter(|t| !t.done).count()
        })
        .sum()
}

pub fn run(g: &Global, a: &FindArgs) -> Result<Output> {
    let s = session::open(g)?;
    let q = Query {
        words: a.words.clone(),
        types: a.types.clone(),
        owners: a.owners.clone(),
        statuses: a.statuses.clone(),
        tags: a.tags.clone(),
        all: a.all,
        text: a.text,
    };
    let mut hits = search::find(&s.corpus, &q);
    let mut open = Vec::new();
    if a.open_tasks {
        hits.retain(|h| {
            let Some(d) = s.corpus.docs.iter().find(|d| d.id.to_string() == h.id) else { return false };
            let files: Vec<&str> =
                std::iter::once(d.path.as_str()).chain(d.chapters.iter().map(String::as_str)).collect();
            let n = open_tasks(&s.corpus.cfg.root, &files);
            if n > 0 {
                open.push(n);
            }
            n > 0
        });
    }
    let mut o = Output::new("find");
    for (i, h) in hits.iter().enumerate() {
        let mut v = to_value(h)?;
        let mut text = format!("{:<36} {:<10} {}", h.id, h.status, h.title);
        if let Some(n) = open.get(i) {
            v["open_tasks"] = (*n).into();
            text.push_str(&format!("  ({n} open)"));
        }
        if let Some(e) = &h.excerpt {
            text.push_str(&format!("\n    {e}"));
        }
        o = o.item(v, text);
    }
    Ok(match hits.first() {
        Some(h) if a.open_tasks => o.next(format!("spec show {} --tasks", h.id)),
        Some(h) => o.next(format!("spec show {}", h.id)),
        None if a.open_tasks => o.foot("no document has open task items"),
        None if !a.all => o.foot("nothing current matches").next("spec find --all ..."),
        None => o.foot("nothing matches"),
    })
}
