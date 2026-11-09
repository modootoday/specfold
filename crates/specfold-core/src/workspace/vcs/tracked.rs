//! Every file git tracks under some directories, submodules included.

use super::repo::{git, split_nul};
use std::path::Path;

/// Tracked files under `bases` (the whole repository when empty), with the files of checked-out
/// submodules. A file the ignore rules match is still listed when it is tracked: it is published.
pub fn tracked(root: &Path, bases: &[String]) -> Option<Vec<String>> {
    let mut args = vec!["ls-files", "-z", "--recurse-submodules", "--"];
    args.extend(bases.iter().map(String::as_str));
    git(root, &args).map(split_nul)
}

/// The files git tracks (index, submodules included) and every folder above one, so a walk can
/// keep to them; a full `spec check` reads nothing else from the repository.
#[derive(Debug, Default)]
pub struct TrackedSet {
    files: std::collections::HashSet<String>,
    dirs: std::collections::HashSet<String>,
}

impl TrackedSet {
    /// None outside a git repository, where every file counts.
    pub fn load(root: &Path) -> Option<Self> {
        let files = tracked(root, &[])?;
        let mut set = TrackedSet::default();
        for f in files {
            let mut dir = f.as_str();
            while let Some((up, _)) = dir.rsplit_once('/') {
                if !set.dirs.insert(up.to_string()) {
                    break;
                }
                dir = up;
            }
            set.files.insert(f);
        }
        Some(set)
    }

    /// Whether git tracks the repository path `rel`.
    pub fn has_file(&self, rel: &str) -> bool {
        self.files.contains(rel)
    }

    /// Whether some tracked file lies below the repository folder `rel`.
    pub fn has_dir(&self, rel: &str) -> bool {
        self.dirs.contains(rel)
    }
}

/// The git blob ids `path` has had in any commit, newest first; empty outside git.
pub fn past_blobs(root: &Path, path: &str) -> Vec<String> {
    let Some(out) = git(root, &["log", "--format=", "--raw", "--no-abbrev", "--", path]) else { return vec![] };
    let mut ids = vec![];
    for line in String::from_utf8_lossy(&out).lines().filter(|l| l.starts_with(':')) {
        let f: Vec<&str> = line.split_whitespace().collect();
        for id in f.iter().skip(2).take(2) {
            if id.chars().any(|c| c != '0') && !ids.contains(&id.to_string()) {
                ids.push(id.to_string());
            }
        }
    }
    ids
}

/// The git blob id of a file's current content.
pub fn blob_id(root: &Path, path: &str) -> Option<String> {
    git(root, &["hash-object", "--", path]).map(|o| String::from_utf8_lossy(&o).trim().to_string())
}

/// Files in the last commit, so callers can tell which documents are new since then.
pub fn committed(root: &Path) -> Option<Vec<String>> {
    git(root, &["ls-tree", "-r", "-z", "--name-only", "HEAD"]).map(split_nul)
}

/// C197: the paths among `paths` that git ignores (and does not track); `git stash` cannot reach them.
pub fn ignored(root: &Path, paths: &[String]) -> Vec<String> {
    if paths.is_empty() {
        return vec![];
    }
    // Paths spec writes have no newline, so git's line output is enough (-z needs --stdin).
    let mut args = vec!["check-ignore", "--"];
    args.extend(paths.iter().map(String::as_str));
    git(root, &args).map(|o| String::from_utf8_lossy(&o).lines().map(str::to_string).collect()).unwrap_or_default()
}
