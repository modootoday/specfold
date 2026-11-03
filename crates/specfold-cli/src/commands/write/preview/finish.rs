//! Shows a plan as a preview with the lines each update changes, or writes it with its undo;
//! and the CLI-7 bound on how many files one write may change.

use super::hunks::hunks;
use super::trash::{STAMP, TRASH, dirty_writes, fresh_bin, keep_untracked, undo};
use crate::output::Output;
use anyhow::Result;
use serde_json::json;
use specfold_core::authoring::{Change, Plan};
use specfold_core::check::finding::visible;
use std::path::Path;

/// Lines shown on each side of a changed region in a preview.
const SHOWN: usize = 4;

/// The plan as it is written: the trash folder gets a fresh name at the write.
fn stamped(plan: &Plan, root: &Path) -> Result<Plan> {
    let mut p = plan.clone();
    let uses = |c: &Change| matches!(c, Change::Move { to, .. } if to.contains(STAMP));
    if !p.changes.iter().any(uses) {
        return Ok(p);
    }
    let now = fresh_bin(root)?;
    for c in p.changes.iter_mut() {
        if let Change::Move { to, .. } = c {
            *to = to.replace(STAMP, &now);
        }
    }
    Ok(p)
}

pub fn finish(mut o: Output, plan: &Plan, root: &Path, write: bool, rerun: &str) -> Result<Output> {
    // The write names the trash folder now; a preview keeps STAMP, so it matches any later write.
    let stamped_plan;
    let plan = if write {
        stamped_plan = stamped(plan, root)?;
        &stamped_plan
    } else {
        plan
    };
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
            // C157: the write keeps the file's line endings, so compare with them set aside.
            // C199: and its leading BOM, which the write keeps too.
            let bare = |t: &str| t.trim_start_matches('\u{FEFF}').replace("\r\n", "\n");
            let hs = hunks(&bare(&old), &bare(content));
            if let Some(first) = hs.first() {
                item["line"] = json!(first.new_at);
                item["hunks"] = json!(
                    hs.iter()
                        .map(|h| json!({"line": h.new_at, "old_line": h.old_at, "before": h.before, "after": h.after}))
                        .collect::<Vec<_>>()
                );
                text = format!("{op:<7} {desc}:{}", first.new_at);
                for h in &hs {
                    // As git writes it: a count of 1 is left out.
                    let side = |at: usize, n: usize| if n == 1 { format!("{at}") } else { format!("{at},{n}") };
                    text.push_str(&format!(
                        "\n  @@ -{} +{} @@",
                        side(h.old_at, h.before.len()),
                        side(h.new_at, h.after.len())
                    ));
                    for (sign, lines) in [("-", &h.before), ("+", &h.after)] {
                        for l in lines.iter().take(SHOWN) {
                            text.push_str(&format!("\n  {sign} {}", visible(l)));
                        }
                        if lines.len() > SHOWN {
                            text.push_str(&format!("\n  {sign} … {} more line(s)", lines.len() - SHOWN));
                        }
                    }
                }
            }
        }
        o = o.item(item, text);
    }
    if write {
        let saved = keep_untracked(plan, root)?;
        plan.apply(root)?;
        let undo = undo(plan, root, &saved);
        Ok(o.field("written", true)
            .field("undo", undo.clone())
            .foot(format!("{} change(s) written", plan.changes.len()))
            .foot(format!("undo: {undo}")))
    } else {
        let dirty = dirty_writes(plan, root);
        if !dirty.is_empty() {
            o = o.field("uncommitted", json!(dirty)).foot(format!(
                "{} file(s) already have uncommitted edits ({}); --write first copies them to {TRASH}/{STAMP}/ and the undo puts that copy back",
                dirty.len(),
                dirty.join(", ")
            ));
        }
        Ok(o.foot("preview only; nothing was written").next(rerun.to_string()).preview())
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
    let o = if write { o.code(crate::output::Code::Refused) } else { o };
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
