//! A successor supersedes what it names when it takes effect: `spec new --from` on a draft records
//! `rel.supersedes` and leaves the old document in force until `spec set` makes the draft hold.

use super::plan::Plan;
use super::set;
use crate::corpus::Corpus;
use crate::model::TypeDef;
use anyhow::Result;
use serde_json::{Map, Value};

/// The documents `fm` supersedes that are still in force: same type, neither superseded nor archived.
pub fn still_in_force(c: &Corpus, ty: &str, fm: &Map<String, Value>) -> Vec<usize> {
    let named: Vec<String> = match fm.get("rel").and_then(|r| r.get("supersedes")) {
        Some(Value::Array(items)) => items.iter().filter_map(Value::as_str).map(str::to_string).collect(),
        Some(Value::String(s)) => vec![s.clone()],
        _ => vec![],
    };
    named
        .iter()
        .map(|n| n.trim().trim_matches(['[', ']']))
        .filter_map(|n| c.docs.iter().position(|d| d.id.ty == ty && d.id.to_string() == n))
        .filter(|&i| !matches!(c.docs[i].status().as_deref(), Some("superseded" | "archived")))
        .collect()
}

/// The writes that mark `olds` superseded now that their successor holds.
pub fn supersede(c: &Corpus, olds: &[usize]) -> Result<Plan> {
    let mut p = Plan::default();
    for &i in olds {
        p.changes.extend(set::plan_superseded_by_new(c, i)?.changes);
    }
    Ok(p)
}

/// The status a pending document of `ty` takes effect in, for the next step to name.
pub fn effective_status(ty: &TypeDef) -> Option<&str> {
    ty.statuses.iter().map(String::as_str).find(|s| ty.transition_allowed(&ty.initial, s) && ty.in_force(s))
}
