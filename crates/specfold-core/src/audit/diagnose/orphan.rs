//! Orphans: active documents that govern nothing and that nothing cites.

use super::super::types::{Item, Kind};
use crate::corpus::Corpus;
use crate::graph::Graph;

pub fn run(c: &Corpus, g: &Graph, docs: &[usize], out: &mut Vec<Item>) {
    for &i in docs {
        let d = &c.docs[i];
        if d.status().as_deref() != Some("active") || !d.rel().get("governs").is_none_or(Vec::is_empty) {
            continue;
        }
        if g.edges_to(i).next().is_none() && g.edges_from(i).next().is_none() {
            out.push(Item {
                kind: Kind::Orphan,
                subject: d.id.to_string(),
                path: d.path.clone(),
                line: 1,
                detail: "active, governs no code, and no document cites it".into(),
                next: Some(format!("spec set {} rel.governs+=<path>   # or: spec set {} status=archived", d.id, d.id)),
            });
        }
    }
}
