//! Exceptions past their review date (the same test as rule T5, listed with the rest of the audit).

use super::super::types::{Item, Kind};
use crate::corpus::Corpus;

pub fn run(c: &Corpus, docs: &[usize], out: &mut Vec<Item>) {
    for (i, date) in crate::check::rules::overdue_exceptions(c) {
        if !docs.contains(&i) {
            continue;
        }
        let d = &c.docs[i];
        let of = d.rel().get("exception_of").cloned().unwrap_or_default().join(", ");
        out.push(Item {
            kind: Kind::Exception,
            subject: d.id.to_string(),
            path: d.path.clone(),
            line: 1,
            detail: format!("exception to {of} was due for review on {date}"),
            next: Some(format!(
                "spec set {} review_by=<YYYY-MM-DD> --write   # or status=rejected to retire it",
                crate::shell::word(&d.id)
            )),
        });
    }
}
