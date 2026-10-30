//! Coverage: anchors of the configured types (`[audit] coverage_types`) that no other document
//! cites as `[[<id>#ANCHOR]]`. A citation to an anchor that no longer exists is L1's job.

use super::super::types::{Item, Kind};
use crate::corpus::Corpus;
use crate::graph::Graph;
use crate::model::lifecycle;
use std::collections::HashSet;

pub fn run(c: &Corpus, g: &Graph, docs: &[usize], out: &mut Vec<Item>) {
    let types = &c.cfg.raw.audit.coverage_types;
    if types.is_empty() {
        return;
    }
    for &i in docs {
        let d = &c.docs[i];
        if !types.contains(&d.id.ty) || !d.status().as_deref().is_some_and(lifecycle::is_current) {
            continue;
        }
        let cited: HashSet<&str> =
            g.edges.iter().filter(|e| e.to == i && e.from != i).filter_map(|e| e.anchor.as_deref()).collect();
        for a in d.anchors.iter().filter(|a| !cited.contains(a.item.as_str())) {
            out.push(Item {
                kind: Kind::Coverage,
                subject: d.id.to_string(),
                path: a.path.clone(),
                line: a.line,
                detail: format!("#{} is cited by no other document", a.item),
                next: Some(format!(
                    "cite it as [[{}#{}]] from the plan, test note or document that covers it",
                    d.id, a.item
                )),
            });
        }
    }
}
