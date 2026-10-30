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
