//! Stalled: active plans without recent updates, and drafts that never became anything.

use super::super::types::{Item, Kind};
use super::days_between;
use crate::corpus::Corpus;
use crate::model::stamp;

pub fn run(c: &Corpus, docs: &[usize], out: &mut Vec<Item>) {
    let today = stamp::today();
    let a = &c.cfg.raw.audit;
    for &i in docs {
        let d = &c.docs[i];
        let status = d.status().unwrap_or_default();
        let (limit, what) = match (d.id.ty.as_str(), status.as_str()) {
            ("plan", "active") => (a.stalled_days, "active plan has no update"),
            (_, "draft") => (a.draft_days, "draft not decided"),
            _ => continue,
        };
        let Some(updated) = d.str_field("updated") else { continue };
        let Some(age) = days_between(&updated, &today) else { continue };
        if age > limit {
            out.push(Item {
                kind: Kind::Stalled,
                subject: d.id.to_string(),
                path: d.path.clone(),
                line: 1,
                detail: format!("{what} for {age} days (since {updated})"),
                next: Some(format!(
                    "{}   # or record progress and spec set {} updated=today",
                    super::archive(c, i),
                    crate::shell::word(&d.id)
                )),
            });
        }
    }
}
