//! Debt: compatibility left over from an import, and rules documents opted out of.

use super::super::types::{Item, Kind};
use crate::corpus::Corpus;
use crate::graph::{Graph, Resolution};

pub fn run(c: &Corpus, g: &Graph, docs: &[usize], out: &mut Vec<Item>) {
    for &i in docs {
        let d = &c.docs[i];
        let alias_cites =
            d.links.iter().filter(|l| matches!(g.resolve(&l.item), Resolution::Doc { via_alias: true, .. })).count();
        if alias_cites > 0 {
            out.push(Item {
                kind: Kind::Debt,
                subject: d.id.to_string(),
                path: d.path.clone(),
                line: 1,
                detail: format!("{alias_cites} references by alias"),
                next: Some(format!("spec fix --rule L1 --write {}", crate::shell::word(&d.path))),
            });
        }
        let skipped = d.list_field("check_skip");
        if !skipped.is_empty() {
            out.push(Item {
                kind: Kind::Debt,
                subject: d.id.to_string(),
                path: d.path.clone(),
                line: 1,
                detail: format!("opts out of {}", skipped.join(", ")),
                next: None,
            });
        }
    }
}
