//! The audit trail of a document, rebuilt from git: when its status and relations changed, in
//! which commit and by whom. git is the record; nothing is logged separately.

use crate::markdown::frontmatter::{self, Parsed};
use crate::model::doc::list_of;
use crate::workspace::vcs;
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct Event {
    pub date: String,
    pub commit: String,
    pub author: String,
    pub change: String,
}

fn snapshot(root: &Path, rev: &vcs::Revision) -> Option<Map<String, Value>> {
    let text = vcs::show(root, &rev.hash, &rev.path)?;
    match frontmatter::parse(frontmatter::split(&text).raw) {
        Parsed::Map(m) => Some(m),
        _ => Some(Map::new()),
    }
}

fn str_of(m: &Map<String, Value>, k: &str) -> String {
    m.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}

fn relations(m: &Map<String, Value>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    if let Some(Value::Object(rel)) = m.get("rel") {
        for (k, v) in rel {
            for t in list_of(Some(v)) {
                out.insert(format!("rel.{k} {t}"));
            }
        }
    }
    out
}

pub fn events(root: &Path, path: &str) -> Vec<Event> {
    let revs = vcs::history(root, path).unwrap_or_default();
    let mut out = vec![];
    let mut prev: Option<Map<String, Value>> = None;
    for rev in &revs {
        let Some(now) = snapshot(root, rev) else { continue };
        let mut changes = vec![];
        match &prev {
            None => changes.push(format!("created ({})", str_of(&now, "status"))),
            Some(p) => {
                let (a, b) = (str_of(p, "status"), str_of(&now, "status"));
                if a != b {
                    changes.push(format!("{a} → {b}"));
                }
                if rev.path != path && str_of(p, "id") != str_of(&now, "id") {
                    changes.push(format!("id {} → {}", str_of(p, "id"), str_of(&now, "id")));
                }
                let (ra, rb) = (relations(p), relations(&now));
                for added in rb.difference(&ra) {
                    changes.push(format!("+ {added}"));
                }
                for removed in ra.difference(&rb) {
                    changes.push(format!("- {removed}"));
                }
            }
        }
        if changes.is_empty() {
            changes.push("edited".into());
        }
        for change in changes {
            out.push(Event {
                date: rev.date.clone(),
                commit: rev.hash.chars().take(10).collect(),
                author: rev.author.clone(),
                change,
            });
        }
        prev = Some(now);
    }
    out
}
