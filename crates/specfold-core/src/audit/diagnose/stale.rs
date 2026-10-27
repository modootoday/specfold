//! Stale: the code a document governs changed long after the document did.

use super::super::types::{Item, Kind};
use super::days_between;
use crate::corpus::Corpus;
use crate::workspace::vcs;

pub fn run(c: &Corpus, docs: &[usize], out: &mut Vec<Item>) {
    let limit = c.cfg.raw.audit.stale_days;
    for &i in docs {
        let d = &c.docs[i];
        if d.status().as_deref() != Some("active") {
            continue;
        }
        let governs = d.rel().remove("governs").unwrap_or_default();
        let (Some(updated), false) = (d.str_field("updated"), governs.is_empty()) else { continue };
        let Some(code) = vcs::last_change(&c.cfg.root, &governs) else { continue };
        let Some(gap) = days_between(&updated, &code) else { continue };
        if gap > limit {
            out.push(Item {
                kind: Kind::Stale,
                subject: d.id.to_string(),
                path: d.path.clone(),
                line: 1,
                detail: format!("governed code changed {code}, {gap} days after the document ({updated})"),
                next: Some(format!("spec show {} && spec log {}", d.id, d.id)),
            });
        }
    }
}
