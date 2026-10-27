//! Review packs: documents that should be read together to judge contradictions and stale
//! statements, grouped so each pack fits a reviewer's context budget.

use crate::corpus::Corpus;
use crate::graph::Graph;
use crate::model::lifecycle;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize)]
pub struct Pack {
    /// What ties the documents together: a governed path, or a document they all cite.
    pub topic: String,
    pub docs: Vec<String>,
    pub paths: Vec<String>,
    pub bytes: u64,
}

fn size(c: &Corpus, i: usize) -> u64 {
    let d = &c.docs[i];
    std::iter::once(&d.path)
        .chain(&d.chapters)
        .filter_map(|p| std::fs::metadata(c.cfg.root.join(p)).ok())
        .map(|m| m.len())
        .sum()
}

/// Groups current documents by the code they govern and by the documents they implement, then
/// splits groups larger than `budget` bytes.
pub fn build(c: &Corpus, g: &Graph, owners: &[String], budget: u64) -> Vec<Pack> {
    let current = |i: usize| {
        c.docs[i].status().as_deref().is_some_and(lifecycle::is_current)
            && (owners.is_empty() || owners.contains(&c.docs[i].id.owner))
    };
    let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for i in (0..c.docs.len()).filter(|&i| current(i)) {
        for gv in c.docs[i].rel().remove("governs").unwrap_or_default() {
            groups.entry(format!("governs {}", crate::placement::literal_base(&gv))).or_default().push(i);
        }
        for e in g.edges_from(i).filter(|e| e.kind == "implements") {
            groups.entry(format!("implements {}", c.docs[e.to].id)).or_default().push(i);
            groups.entry(format!("implements {}", c.docs[e.to].id)).or_default().push(e.to);
        }
    }
    let mut packs = vec![];
    for (topic, mut members) in groups {
        members.sort();
        members.dedup();
        if members.len() < 2 {
            continue;
        }
        let mut cur: Vec<usize> = vec![];
        let mut bytes = 0;
        for i in members {
            let s = size(c, i);
            if !cur.is_empty() && bytes + s > budget {
                packs.push(finish(c, &topic, &cur, bytes));
                cur.clear();
                bytes = 0;
            }
            cur.push(i);
            bytes += s;
        }
        if cur.len() > 1 {
            packs.push(finish(c, &topic, &cur, bytes));
        }
    }
    packs
}

fn finish(c: &Corpus, topic: &str, members: &[usize], bytes: u64) -> Pack {
    Pack {
        topic: topic.to_string(),
        docs: members.iter().map(|&i| c.docs[i].id.to_string()).collect(),
        paths: members.iter().map(|&i| c.docs[i].path.clone()).collect(),
        bytes,
    }
}
