//! Gaps: owners whose code has no active source of truth.

use super::super::types::{Item, Kind};
use crate::corpus::Corpus;

pub fn run(c: &Corpus, owners: &[String], out: &mut Vec<Item>) {
    for o in c.owners.list.iter().filter(|o| !o.path.is_empty() && !o.retired && owners.contains(&o.id)) {
        let stored = c.owners.storage_owner(o);
        let has_rule = c.docs.iter().any(|d| {
            d.id.ty == "sot"
                && d.status().as_deref() == Some("active")
                && (d.id.owner == o.id || d.id.owner == stored.id)
        });
        if !has_rule {
            out.push(Item {
                kind: Kind::Gap,
                subject: o.id.clone(),
                path: o.path.clone(),
                line: 0,
                detail: "has code but no active sot".into(),
                next: Some(format!("spec new sot --governs {} --title '…'", o.path)),
            });
        }
    }
}
