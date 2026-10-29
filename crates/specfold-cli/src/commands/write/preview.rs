//! Shows a plan, and applies it only when asked.

use crate::output::Output;
use anyhow::Result;
use serde_json::json;
use specfold_core::authoring::{Change, Plan};
use specfold_core::check::finding::visible;
use std::path::Path;

/// Lines shown on each side of a changed region in a preview.
const SHOWN: usize = 4;

/// The changed region between two texts: the first changed line (1-based) and the lines it had
/// before and has after, once the common start and end are set aside.
fn changed_lines(old: &str, new: &str) -> (usize, Vec<String>, Vec<String>) {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    let head = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let tail = a[head..].iter().rev().zip(b[head..].iter().rev()).take_while(|(x, y)| x == y).count();
    let before: Vec<String> = a[head..a.len() - tail].iter().map(|s| s.to_string()).collect();
    let after: Vec<String> = b[head..b.len() - tail].iter().map(|s| s.to_string()).collect();
    (head + 1, before, after)
}

pub fn finish(mut o: Output, plan: &Plan, root: &Path, write: bool, rerun: &str) -> Result<Output> {
    for c in &plan.changes {
        let (op, desc) = match c {
            Change::Write { path, created: true, .. } => ("create", path.clone()),
            Change::Write { path, .. } => ("update", path.clone()),
            Change::Move { from, to } => ("move", format!("{from} -> {to}")),
            Change::Copy { from, to } => ("copy", format!("{from} -> {to}")),
            Change::Remove { path } => ("remove", path.clone()),
        };
        let mut item = json!({"op": op, "path": desc});
        let mut text = format!("{op:<7} {desc}");
        // An update shows the lines it changes, so a preview can be judged before --write.
        if let Change::Write { path, content, created: false } = c
            && !write
            && let Ok(old) = std::fs::read_to_string(root.join(path))
        {
            let (at, before, after) = changed_lines(&old, content);
            if !before.is_empty() || !after.is_empty() {
                item["line"] = json!(at);
                item["before"] = json!(before);
                item["after"] = json!(after);
                for (sign, lines) in [("-", &before), ("+", &after)] {
                    for l in lines.iter().take(SHOWN) {
                        text.push_str(&format!("\n  {sign} {}", visible(l)));
                    }
                    if lines.len() > SHOWN {
                        text.push_str(&format!("\n  {sign} … {} more line(s)", lines.len() - SHOWN));
                    }
                }
                if at > 0 {
                    text = text.replacen(&format!("{op:<7} {desc}"), &format!("{op:<7} {desc}:{at}"), 1);
                }
            }
        }
        o = o.item(item, text);
    }
    if write {
        plan.apply(root)?;
        Ok(o.field("written", true).foot(format!("{} change(s) written", plan.changes.len())))
    } else {
        Ok(o.field("written", false).foot("preview only; nothing was written").next(rerun.to_string()))
    }
}
