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
