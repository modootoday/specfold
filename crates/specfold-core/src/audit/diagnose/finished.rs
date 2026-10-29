//! Plans whose task list and status disagree: every task ticked but still active, or archived
//! with tasks left open.

use super::super::types::{Item, Kind};
use crate::corpus::Corpus;
use crate::markdown::{frontmatter, tasks};

pub fn run(c: &Corpus, docs: &[usize], out: &mut Vec<Item>) {
    for &i in docs {
        let d = &c.docs[i];
        if d.id.ty != "plan" {
            continue;
        }
        let status = d.status().unwrap_or_default();
        let (mut done, mut total) = (0, 0);
        for file in std::iter::once(&d.path).chain(&d.chapters) {
            let Ok(text) = std::fs::read_to_string(c.cfg.root.join(file)) else { continue };
            for t in tasks::list(frontmatter::split(&text).body) {
                total += 1;
                done += usize::from(t.done);
            }
        }
        if total == 0 {
            continue;
        }
        let (detail, next) = match status.as_str() {
            "active" if done == total => (
                format!("all {total} tasks are done but the plan is still active"),
                format!("spec set {} status=archived   # or add the work that is left", d.id),
            ),
            "archived" if done < total => (
                format!("archived with {} of {total} tasks still open", total - done),
                format!("spec show {} --tasks   # tick them, or note why they were dropped", d.id),
            ),
            _ => continue,
        };
        out.push(Item {
            kind: Kind::Finished,
            subject: d.id.to_string(),
            path: d.path.clone(),
            line: 1,
            detail,
            next: Some(next),
        });
    }
}
