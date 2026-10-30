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
        let undo = undo(plan);
        Ok(o.field("written", true)
            .field("undo", undo.clone())
            .foot(format!("{} change(s) written", plan.changes.len()))
            .foot(format!("undo: {undo}")))
    } else {
        Ok(o.field("written", false).foot("preview only; nothing was written").next(rerun.to_string()))
    }
}

/// The files a plan touches (both ends of a move or copy).
pub fn files(plan: &Plan) -> usize {
    let mut paths = std::collections::BTreeSet::new();
    for c in &plan.changes {
        match c {
            Change::Write { path, .. } | Change::Remove { path } => {
                paths.insert(path.clone());
            }
            Change::Move { from, to } | Change::Copy { from, to } => {
                paths.insert(from.clone());
                paths.insert(to.clone());
            }
        }
    }
    paths.len()
}

/// CLI-7: the output and whether `--write` may go ahead. Over `[write] max_files` (0: no bound),
/// a write needs `--yes`; the preview says how many files and the next command adds `--yes`.
pub fn bound(o: Output, n: usize, max: usize, write: bool, yes: bool) -> (Output, bool) {
    if max == 0 || n <= max || yes {
        return (o, write);
    }
    let o = o.field("files", n).field("max_files", max).field("bounded", true).head(format!(
        "{n} files would change, over [write] max_files = {max}: {}narrow the scope or add --yes",
        if write { "nothing was written; " } else { "" }
    ));
    (o, false)
}

/// `finish` with the CLI-7 bound: returns the output and whether the plan was written.
pub fn finish_bounded(
    o: Output,
    plan: &Plan,
    root: &Path,
    max: usize,
    write: bool,
    yes: bool,
    rerun: &str,
) -> Result<(Output, bool)> {
    let (o, go) = bound(o, files(plan), max, write, yes);
    let over = o.summary.contains_key("bounded");
    let rerun = if over { format!("{rerun} --yes") } else { rerun.to_string() };
    Ok((finish(o, plan, root, go, &rerun)?, go))
}

/// CLI-5: a git command that takes back what a write changed, new files included; it keeps the
/// changes in a stash, so the undo itself can be undone with `git stash pop`.
/// A file moved here instead of deleted comes back with a copy, since git may never have had it.
pub const TRASH: &str = ".specfold/trash";

pub fn undo(plan: &Plan) -> String {
    let mut paths: Vec<String> = vec![];
    let mut trash: Vec<String> = vec![];
    for c in &plan.changes {
        match c {
            Change::Write { path, .. } | Change::Remove { path } => paths.push(path.clone()),
            Change::Move { to, .. } if to.starts_with(&format!("{TRASH}/")) => {
                // .specfold/trash/<stamp>/<path>: restoring the stamp directory puts every file back.
                let dir: Vec<&str> = to.splitn(4, '/').take(3).collect();
                trash.push(dir.join("/"));
            }
            Change::Move { from, to } => {
                paths.push(from.clone());
                paths.push(to.clone());
            }
            // A copy changes only its target; its source stays the user's.
            Change::Copy { to, .. } => paths.push(to.clone()),
        }
    }
    paths.sort();
    paths.dedup();
    trash.sort();
    trash.dedup();
    let mut parts = vec![];
    if !paths.is_empty() {
        let words: Vec<String> = paths.iter().map(|p| crate::session::shell_word(p)).collect();
        parts.push(format!("git stash push --include-untracked -- {}", words.join(" ")));
    }
    for t in &trash {
        parts.push(format!("cp -Rp {}/. .", crate::session::shell_word(t)));
    }
    parts.join(" && ")
}
