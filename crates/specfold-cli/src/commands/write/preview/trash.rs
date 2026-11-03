//! What a write keeps before it runs, in a fresh trash folder, and the command that takes the
//! write back.

use anyhow::Result;
use specfold_core::authoring::{Change, Plan};
use std::path::Path;

/// Sync and other trash users name the trash folder with this until the write: a preview cannot
/// know the second the write will run in, so it shows the same path with this in its place.
pub const STAMP: &str = "<timestamp>";

/// C187: a trash folder no other write has: the second of the write, then `-1`, `-2`, … when
/// that folder exists already. Creating it is the claim, so two writes in one second (or two
/// processes) never share a folder and an earlier copy is never overwritten.
pub fn fresh_bin(root: &Path) -> Result<String> {
    let base = root.join(TRASH);
    std::fs::create_dir_all(&base)?;
    let now = specfold_core::model::stamp::now_stamp();
    for n in 0u32.. {
        let name = if n == 0 { now.clone() } else { format!("{now}-{n}") };
        match std::fs::create_dir(base.join(&name)) {
            Ok(()) => return Ok(name),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        }
    }
    anyhow::bail!("no free folder name under {TRASH}")
}

/// CLI-5: a command that takes back what a write changed: `rm -rf` for files it created and
/// `git restore` for tracked files (E2: never the stash, which every worktree shares).
/// A file moved here instead of deleted comes back with a copy, since git may never have had it.
pub const TRASH: &str = ".specfold/trash";

/// C122: git cannot give back a document it never had, so before a move of an untracked file a
/// copy goes to the trash directory, and the undo copies it back (content included).
/// What a write kept before it ran: the trash folder holding the copies, and which of them are
/// tracked files that already had uncommitted edits (C183).
#[derive(Default)]
pub struct Saved {
    pub bin: Option<String>,
    pub dirty: Vec<String>,
}

/// C183: tracked files a plan updates that already differ from HEAD. A `git restore` undo would
/// take the user's own edits away with spec's, so these are copied to the trash instead.
pub fn dirty_writes(plan: &Plan, root: &Path) -> Vec<String> {
    let tracked: std::collections::HashSet<String> =
        specfold_core::workspace::vcs::tracked(root, &[]).unwrap_or_default().into_iter().collect();
    let changed = specfold_core::workspace::vcs::changed_since(root, "HEAD").unwrap_or_default();
    let mut out: Vec<String> = plan
        .changes
        .iter()
        .filter_map(|c| match c {
            Change::Write { path, created: false, .. } if tracked.contains(path) && changed.contains(path) => {
                Some(path.clone())
            }
            _ => None,
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

/// C209: files a plan rewrites that git cannot give back, the dirty tracked ones and the ones
/// never committed; their pre-write copy goes to the trash.
fn unrecoverable_writes(plan: &Plan, root: &Path) -> Vec<String> {
    let tracked: std::collections::HashSet<String> =
        specfold_core::workspace::vcs::tracked(root, &[]).unwrap_or_default().into_iter().collect();
    let mut out = dirty_writes(plan, root);
    for c in &plan.changes {
        if let Change::Write { path, created: false, .. } = c
            && !tracked.contains(path)
            && root.join(path).is_file()
        {
            out.push(path.clone());
        }
    }
    out.sort();
    out.dedup();
    out
}

pub fn keep_untracked(plan: &Plan, root: &Path) -> Result<Saved> {
    let dirty = unrecoverable_writes(plan, root);
    let tracked: std::collections::HashSet<String> =
        specfold_core::workspace::vcs::tracked(root, &[]).unwrap_or_default().into_iter().collect();
    let mut bin: Option<String> = None;
    for c in &plan.changes {
        let from = match c {
            Change::Move { from, to } if !to.starts_with(&format!("{TRASH}/")) => from,
            // C123: an import removes its sources; one never committed has no other copy.
            Change::Remove { path } => path,
            Change::Write { path, .. } if dirty.contains(path) => path,
            _ => continue,
        };
        if (!tracked.contains(from) || dirty.contains(from)) && root.join(from).is_file() {
            if bin.is_none() {
                bin = Some(format!("{TRASH}/{}", fresh_bin(root)?));
            }
            let dst = root.join(bin.as_deref().unwrap_or_default()).join(from);
            std::fs::create_dir_all(dst.parent().expect("a file path has a parent"))?;
            std::fs::copy(root.join(from), dst)?;
        }
    }
    Ok(Saved { bin, dirty })
}

pub fn undo(plan: &Plan, root: &Path, kept: &Saved) -> String {
    let saved = kept.bin.as_deref();
    // C122: git knows only tracked paths and files on disk; a path that is neither (the old end
    // of a move of a document not yet committed) would make the stash fail, so that move is
    // taken back with a plain `mv` instead.
    let tracked: std::collections::HashSet<String> =
        specfold_core::workspace::vcs::tracked(root, &[]).unwrap_or_default().into_iter().collect();
    // C233: a bundle document moves as its directory, which git never lists itself; it counts
    // as tracked when files under it are.
    let under = |p: &str| tracked.iter().any(|t| specfold_core::paths::is_below(t, p));
    let is_tracked = |p: &str| tracked.contains(p) || under(p);
    let known = |p: &str| is_tracked(p) || root.join(p).exists();
    let mut paths: Vec<String> = vec![];
    let mut trash: Vec<String> = vec![];
    let mut back: Vec<(String, String)> = vec![];
    let mut lost = false;
    for c in &plan.changes {
        match c {
            Change::Remove { path } if !tracked.contains(path) => lost = true,
            Change::Write { path, .. } if kept.dirty.contains(path) => lost = true,
            Change::Write { path, .. } | Change::Remove { path } => paths.push(path.clone()),
            Change::Move { to, .. } if to.starts_with(&format!("{TRASH}/")) => {
                // .specfold/trash/<stamp>/<path>: restoring the stamp directory puts every file back.
                let dir: Vec<&str> = to.splitn(4, '/').take(3).collect();
                trash.push(dir.join("/"));
            }
            Change::Move { from, to } if !is_tracked(from) => back.push((to.clone(), from.clone())),
            Change::Move { from, to } => {
                paths.push(from.clone());
                paths.push(to.clone());
            }
            // A copy changes only its target; its source stays the user's.
            Change::Copy { to, .. } => paths.push(to.clone()),
        }
    }
    paths.retain(|p| known(p) && !back.iter().any(|(to, _)| specfold_core::paths::is_under(p, to)));
    paths.sort();
    paths.dedup();
    // C197: an ignored path (the local tier) is out of git's reach: what spec made there is removed.
    let ignored = specfold_core::workspace::vcs::ignored(root, &paths);
    paths.retain(|p| !ignored.contains(p));
    trash.sort();
    trash.dedup();
    let w = specfold_core::shell::quote;
    let mut parts = vec![];
    for p in &ignored {
        parts.push(format!("rm -f -- {}", w(p)));
    }
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
    // E2: no stash (shared by every worktree): tracked files come back from the index, and
    // files this write created are removed.
    let (old_files, made): (Vec<&String>, Vec<&String>) = paths.iter().partition(|p| is_tracked(p));
    if !made.is_empty() {
        let words: Vec<String> = made.iter().map(|p| w(p)).collect();
        parts.push(format!("rm -rf -- {}", words.join(" ")));
    }
    if !old_files.is_empty() {
        let words: Vec<String> = old_files.iter().map(|p| w(p)).collect();
        parts.push(format!("git restore -- {}", words.join(" ")));
    }
    for t in &trash {
        parts.push(format!("cp -Rp {}/. .", w(t)));
    }
    parts.join(" && ")
}
