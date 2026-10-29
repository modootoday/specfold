//! Stale: the code a document governs changed long after the document did, or, for a document
//! pinned with `verified_at: <commit>`, changed in any commit since the pin.

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
        if governs.is_empty() {
            continue;
        }
        if let Some(pin) = d.str_field("verified_at") {
            // Commits to documents are not changes to the code they describe, and the pin's own
            // commit would otherwise always count.
            let docs = vec![format!("**/{}/**", c.cfg.spec_dir()), format!("{}/**", c.cfg.spec_dir())];
            let detail = match vcs::commits_since(&c.cfg.root, &pin, &governs, &docs) {
                None => format!("verified_at {pin} is not a commit in this repository"),
                Some(list) if list.is_empty() => continue,
                Some(list) => format!(
                    "governed code changed in {} commit(s) since verified_at {pin} (latest {})",
                    list.len(),
                    list[0]
                ),
            };
            out.push(Item {
                kind: Kind::Stale,
                subject: d.id.to_string(),
                path: d.path.clone(),
                line: 1,
                detail,
                next: Some(format!(
                    "git diff {pin}..HEAD -- {}  # then update the body and: spec set {} verified_at=HEAD",
                    governs.join(" "),
                    d.id
                )),
            });
            continue;
        }
        let Some(updated) = d.str_field("updated") else { continue };
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
