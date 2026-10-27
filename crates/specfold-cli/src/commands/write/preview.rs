//! Shows a plan, and applies it only when asked.

use crate::output::Output;
use anyhow::Result;
use serde_json::json;
use specfold_core::authoring::{Change, Plan};
use std::path::Path;

pub fn finish(mut o: Output, plan: &Plan, root: &Path, write: bool, rerun: &str) -> Result<Output> {
    for c in &plan.changes {
        let (op, desc) = match c {
            Change::Write { path, created: true, .. } => ("create", path.clone()),
            Change::Write { path, .. } => ("update", path.clone()),
            Change::Move { from, to } => ("move", format!("{from} -> {to}")),
            Change::Copy { from, to } => ("copy", format!("{from} -> {to}")),
            Change::Remove { path } => ("remove", path.clone()),
        };
        o = o.item(json!({"op": op, "path": desc}), format!("{op:<7} {desc}"));
    }
    if write {
        plan.apply(root)?;
        Ok(o.field("written", true).foot(format!("{} change(s) written", plan.changes.len())))
    } else {
        Ok(o.field("written", false).foot("preview only; nothing was written").next(rerun.to_string()))
    }
}
