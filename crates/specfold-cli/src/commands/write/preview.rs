//! Shows a plan, and applies it only when asked.

use crate::output::Output;
use anyhow::Result;
use serde_json::json;
use specfold_core::authoring::{Change, Plan};
use specfold_core::check::finding::visible;
use std::path::Path;

/// Lines shown on each side of a changed region in a preview.
const SHOWN: usize = 4;

/// One changed region: where it starts in the old and new text (1-based) and its lines.
struct Hunk {
    old_at: usize,
    new_at: usize,
    before: Vec<String>,
    after: Vec<String>,
}

/// The changed regions between two texts, as a unified diff splits them: changes with an
/// unchanged line between them are separate hunks, so a refreshed `updated:` line and an edited
/// section far below it are shown apart rather than as one block spanning both.
fn hunks(old: &str, new: &str) -> Vec<Hunk> {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    // Common start and end first: the table below is then only as big as the changed middle.
    let head = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let tail = a[head..].iter().rev().zip(b[head..].iter().rev()).take_while(|(x, y)| x == y).count();
    let (x, y) = (&a[head..a.len() - tail], &b[head..b.len() - tail]);
    // Longest common subsequence over the middle.
    let mut l = vec![vec![0usize; y.len() + 1]; x.len() + 1];
    for i in (0..x.len()).rev() {
        for j in (0..y.len()).rev() {
            l[i][j] = if x[i] == y[j] { l[i + 1][j + 1] + 1 } else { l[i + 1][j].max(l[i][j + 1]) };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut out: Vec<Hunk> = vec![];
    let mut open = false;
    while i < x.len() || j < y.len() {
        if i < x.len() && j < y.len() && x[i] == y[j] {
            open = false;
            i += 1;
            j += 1;
            continue;
        }
        if !open {
            out.push(Hunk { old_at: head + i + 1, new_at: head + j + 1, before: vec![], after: vec![] });
            open = true;
        }
        let h = out.last_mut().expect("a hunk is open");
        if j < y.len() && (i == x.len() || l[i][j + 1] >= l[i + 1][j]) {
            h.after.push(y[j].to_string());
            j += 1;
        } else {
            h.before.push(x[i].to_string());
            i += 1;
        }
    }
    out
}

/// Sync and other trash users name the trash folder with this until the write: a preview cannot
/// know the second the write will run in, so it shows the same path with this in its place.
pub const STAMP: &str = "<timestamp>";

/// The plan as it is written: the trash folder gets the second of the write.
fn stamped(plan: &Plan) -> Plan {
    let now = specfold_core::model::stamp::now_stamp();
    let mut p = plan.clone();
    for c in p.changes.iter_mut() {
        if let Change::Move { to, .. } = c {
            *to = to.replace(STAMP, &now);
        }
    }
    p
}

pub fn finish(mut o: Output, plan: &Plan, root: &Path, write: bool, rerun: &str) -> Result<Output> {
    // The write names the trash folder now; a preview keeps STAMP, so it matches any later write.
    let stamped_plan;
    let plan = if write {
        stamped_plan = stamped(plan);
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
            let hs = hunks(&old.replace("\r\n", "\n"), &content.replace("\r\n", "\n"));
            if let Some(first) = hs.first() {
                item["line"] = json!(first.new_at);
                item["hunks"] = json!(
                    hs.iter()
                        .map(|h| json!({"line": h.new_at, "old_line": h.old_at, "before": h.before, "after": h.after}))
                        .collect::<Vec<_>>()
                );
                text = format!("{op:<7} {desc}:{}", first.new_at);
                for h in &hs {
                    text.push_str(&format!(
                        "\n  @@ -{},{} +{},{} @@",
                        h.old_at,
                        h.before.len(),
                        h.new_at,
                        h.after.len()
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
        let undo = undo(plan, root, saved.as_deref());
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

/// CLI-5: a git command that takes back what a write changed, new files included; it keeps the
/// changes in a stash, so the undo itself can be undone with `git stash pop`.
/// A file moved here instead of deleted comes back with a copy, since git may never have had it.
pub const TRASH: &str = ".specfold/trash";

/// C122: git cannot give back a document it never had, so before a move of an untracked file a
/// copy goes to the trash directory, and the undo copies it back (content included).
pub fn keep_untracked(plan: &Plan, root: &Path) -> Result<Option<String>> {
    let tracked: std::collections::HashSet<String> =
        specfold_core::workspace::vcs::tracked(root, &[]).unwrap_or_default().into_iter().collect();
    let bin = format!("{TRASH}/{}", specfold_core::model::stamp::now_stamp());
    let mut any = false;
    for c in &plan.changes {
        let from = match c {
            Change::Move { from, to } if !to.starts_with(&format!("{TRASH}/")) => from,
            // C123: an import removes its sources; one never committed has no other copy.
            Change::Remove { path } => path,
            _ => continue,
        };
        if !tracked.contains(from) && root.join(from).is_file() {
            let dst = root.join(&bin).join(from);
            std::fs::create_dir_all(dst.parent().expect("a file path has a parent"))?;
            std::fs::copy(root.join(from), dst)?;
            any = true;
        }
    }
    Ok(any.then_some(bin))
}

pub fn undo(plan: &Plan, root: &Path, saved: Option<&str>) -> String {
    // C122: git knows only tracked paths and files on disk; a path that is neither (the old end
    // of a move of a document not yet committed) would make the stash fail, so that move is
    // taken back with a plain `mv` instead.
    let tracked: std::collections::HashSet<String> =
        specfold_core::workspace::vcs::tracked(root, &[]).unwrap_or_default().into_iter().collect();
    let known = |p: &str| tracked.contains(p) || root.join(p).exists();
    let mut paths: Vec<String> = vec![];
    let mut trash: Vec<String> = vec![];
    let mut back: Vec<(String, String)> = vec![];
    let mut lost = false;
    for c in &plan.changes {
        match c {
            Change::Remove { path } if !tracked.contains(path) => lost = true,
            Change::Write { path, .. } | Change::Remove { path } => paths.push(path.clone()),
            Change::Move { to, .. } if to.starts_with(&format!("{TRASH}/")) => {
                // .specfold/trash/<stamp>/<path>: restoring the stamp directory puts every file back.
                let dir: Vec<&str> = to.splitn(4, '/').take(3).collect();
                trash.push(dir.join("/"));
            }
            Change::Move { from, to } if !tracked.contains(from) => back.push((to.clone(), from.clone())),
            Change::Move { from, to } => {
                paths.push(from.clone());
                paths.push(to.clone());
            }
            // A copy changes only its target; its source stays the user's.
            Change::Copy { to, .. } => paths.push(to.clone()),
        }
    }
    paths.retain(|p| known(p) && !back.iter().any(|(to, _)| to == p));
    paths.sort();
    paths.dedup();
    trash.sort();
    trash.dedup();
    let w = crate::session::shell_word;
    let mut parts = vec![];
    for (to, _) in back.iter().filter(|_| saved.is_some()) {
        parts.push(format!("rm -f -- {}", w(to)));
    }
    if let Some(bin) = saved.filter(|_| !back.is_empty() || lost) {
        parts.push(format!("cp -Rp {}/. .", w(bin)));
    }
    for (to, from) in back.iter().filter(|_| saved.is_none()) {
        if let Some(dir) = Path::new(from).parent().filter(|d| !d.as_os_str().is_empty()) {
            parts.push(format!("mkdir -p {}", w(&dir.to_string_lossy())));
        }
        parts.push(format!("mv -- {} {}", w(to), w(from)));
    }
    if !paths.is_empty() {
        let words: Vec<String> = paths.iter().map(|p| w(p)).collect();
        parts.push(format!("git stash push --include-untracked -- {}", words.join(" ")));
    }
    for t in &trash {
        parts.push(format!("cp -Rp {}/. .", w(t)));
    }
    parts.join(" && ")
}
