//! What a change touches inside submodules: a gitlink that moves stands for the files that
//! changed between its two commits, which only the submodule's own checkout can list.

use super::repo::{git, split_nul};
use std::path::Path;

/// Which change of the outer repository to read gitlinks from.
#[derive(Debug, Clone, Copy)]
pub enum Side<'a> {
    /// The index against HEAD.
    Staged,
    /// The working tree against a base revision.
    Since(&'a str),
}

const ZERO: &str = "0000000000000000000000000000000000000000";

/// Repository-relative paths changed inside submodules whose gitlink this change adds or moves.
/// A submodule that is not checked out, or that lacks either commit, adds nothing.
pub fn submodule_changes(root: &Path, side: Side) -> Vec<String> {
    expand(root, side).files
}

/// C1: what the moved gitlinks of a change stand for.
#[derive(Debug, Default)]
pub struct Expansion {
    /// Repository-relative files changed inside the submodules.
    pub files: Vec<String>,
    /// Gitlinks whose old..new was read, so the pointer itself need not count as a change.
    pub expanded: Vec<String>,
    /// Gitlinks whose checkout or commits are missing; the pointer stands for the whole directory.
    pub missing: Vec<String>,
    /// Every expanded gitlink: a new one (a split, when files of the outer repository moved into
    /// it) or a pin bump, with what it holds back from the change.
    pub moves: Vec<GitlinkMove>,
}

/// One gitlink a change adds or moves, read as one event rather than as its files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitlinkMove {
    pub path: String,
    /// The commit pinned before; None for a gitlink the change adds.
    pub old: Option<String>,
    pub new: String,
    /// Repository-relative files that changed inside it (in `Expansion.files`).
    pub changed: Vec<String>,
    /// For an added gitlink: files the outer repository held at the same path with the same
    /// content, so the change moved them rather than deleting and adding them.
    pub moved: Vec<String>,
}

impl GitlinkMove {
    /// Whether the change adds this gitlink over files that were in the outer repository.
    pub fn is_split(&self) -> bool {
        self.old.is_none() && !self.moved.is_empty()
    }
}

/// Every moved gitlink of a change, expanded into the files changed inside it where possible.
pub fn expand(root: &Path, side: Side) -> Expansion {
    let mut args = vec!["diff", "--raw", "-z", "--no-abbrev", "--diff-filter=AM"];
    let base = match side {
        Side::Staged => {
            args.push("--cached");
            "HEAD"
        }
        Side::Since(base) => {
            args.push(base);
            base
        }
    };
    let Some(out) = git(root, &args) else { return Expansion::default() };
    let fields = split_nul(out);
    let mut e = Expansion::default();
    for pair in fields.chunks(2) {
        let [meta, path] = pair else { continue };
        let parts: Vec<&str> = meta.trim_start_matches(':').split(' ').collect();
        let [old_mode, new_mode, old, new, _status] = parts[..] else { continue };
        if new_mode != "160000" {
            continue;
        }
        let sub = root.join(path);
        let added = old_mode != "160000" || old == ZERO;
        let read =
            sub.join(".git").exists().then(|| inner_changes(root, &sub, (path, base), added, old, new)).flatten();
        match read {
            Some(m) => {
                e.files.extend(m.changed.iter().cloned());
                e.expanded.push(path.clone());
                e.moves.push(m);
            }
            None => e.missing.push(path.clone()),
        }
    }
    e.files.sort();
    e.files.dedup();
    e
}

/// What one submodule's gitlink stands for: the files changed between the recorded commit and
/// the new one, or for a new gitlink every file in it, less those the outer repository held at
/// `base` with the same blob. A working-tree side has no recorded commit, so HEAD stands for it.
fn inner_changes(
    root: &Path,
    sub: &Path,
    (path, base): (&str, &str),
    added: bool,
    old: &str,
    new: &str,
) -> Option<GitlinkMove> {
    let new = if new == ZERO {
        String::from_utf8_lossy(&git(sub, &["rev-parse", "HEAD"])?).trim().to_string()
    } else {
        new.to_string()
    };
    let at = |p: &str| format!("{path}/{p}");
    if !added {
        let listed = git(sub, &["diff", "--name-only", "-z", "--diff-filter=ACMR", old, &new])?;
        let changed = split_nul(listed).iter().map(|p| at(p)).collect();
        return Some(GitlinkMove { path: path.into(), old: Some(old.into()), new, changed, moved: vec![] });
    }
    let inner = blobs(sub, &new, None)?;
    let outer: std::collections::HashMap<String, String> =
        blobs(root, base, Some(&format!("{path}/"))).unwrap_or_default().into_iter().collect();
    let (mut changed, mut moved) = (vec![], vec![]);
    for (p, blob) in inner {
        let p = at(&p);
        if outer.get(&p) == Some(&blob) { moved.push(p) } else { changed.push(p) }
    }
    Some(GitlinkMove { path: path.into(), old: None, new, changed, moved })
}

/// `(path, blob)` of every file in `rev`'s tree, under `prefix` when given.
fn blobs(repo: &Path, rev: &str, prefix: Option<&str>) -> Option<Vec<(String, String)>> {
    let mut args = vec!["ls-tree", "-r", "-z", "--full-tree", rev];
    if let Some(p) = prefix {
        args.extend(["--", p]);
    }
    let entries = split_nul(git(repo, &args)?);
    Some(
        entries
            .iter()
            .filter_map(|e| {
                let (meta, p) = e.split_once('\t')?;
                let blob = meta.split(' ').nth(2)?;
                Some((p.to_string(), blob.to_string()))
            })
            .collect(),
    )
}

/// Whether the commit gitlink `link` pins holds `rest` (a path inside it): None when that cannot
/// be told, because the submodule is not checked out or lacks the commit.
pub fn gitlink_holds(root: &Path, link: &str, rest: &str) -> Option<bool> {
    let sub = root.join(link);
    if !sub.join(".git").exists() {
        return None;
    }
    let pin = String::from_utf8_lossy(&git(root, &["rev-parse", &format!("HEAD:{link}")])?).trim().to_string();
    git(&sub, &["cat-file", "-e", &format!("{pin}^{{commit}}")])?;
    Some(git(&sub, &["cat-file", "-e", &format!("{pin}:{rest}")]).is_some())
}

/// Repository-relative paths of every gitlink (submodule) the index records.
pub fn gitlinks(root: &Path) -> Vec<String> {
    let Some(out) = git(root, &["ls-files", "-s", "-z"]) else { return vec![] };
    split_nul(out)
        .into_iter()
        .filter(|e| e.starts_with("160000 "))
        .filter_map(|e| e.split_once('\t').map(|(_, p)| p.to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn run(dir: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.email=t@t", "-c", "user.name=t", "-c", "protocol.file.allow=always"])
            .args(args)
            .output()
            .unwrap()
            .status
            .success();
        assert!(ok, "git {args:?}");
    }

    #[test]
    fn a_moved_gitlink_lists_the_files_changed_inside_it() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = tmp.path().join("lib");
        std::fs::create_dir_all(&lib).unwrap();
        run(&lib, &["init", "-q"]);
        std::fs::write(lib.join("a.md"), "a").unwrap();
        run(&lib, &["add", "a.md"]);
        run(&lib, &["commit", "-qm", "a"]);
        let outer = tmp.path().join("outer");
        std::fs::create_dir_all(&outer).unwrap();
        run(&outer, &["init", "-q"]);
        run(&outer, &["submodule", "add", "-q", lib.to_str().unwrap(), "libs/lib"]);
        run(&outer, &["commit", "-qm", "pin"]);
        let sub = outer.join("libs/lib");
        std::fs::create_dir_all(sub.join(".spec/adr")).unwrap();
        std::fs::write(sub.join(".spec/adr/new.md"), "n").unwrap();
        run(&sub, &["add", ".spec/adr/new.md"]);
        run(&sub, &["commit", "-qm", "new"]);
        run(&outer, &["add", "libs/lib"]);
        assert_eq!(submodule_changes(&outer, Side::Staged), vec!["libs/lib/.spec/adr/new.md".to_string()]);
        assert_eq!(submodule_changes(&outer, Side::Since("HEAD")), vec!["libs/lib/.spec/adr/new.md".to_string()]);
        assert_eq!(expand(&outer, Side::Staged).expanded, vec!["libs/lib".to_string()]);
        std::fs::remove_file(sub.join(".git")).unwrap();
        let e = expand(&outer, Side::Staged);
        assert_eq!((e.files.len(), e.missing), (0, vec!["libs/lib".to_string()]));
    }

    #[test]
    fn an_unmoved_gitlink_adds_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        run(tmp.path(), &["init", "-q"]);
        assert!(submodule_changes(tmp.path(), Side::Staged).is_empty());
    }
}
