//! Lists the files under a spec directory.

use crate::model::Tier;
use ignore::WalkBuilder;
use std::path::Path;

/// Repository-relative paths of every file under `dir`. Committed roots honour `.gitignore`;
/// the local root is ignored by git on purpose, so it is read as is.
pub fn files(repo_root: &Path, dir: &str, tier: Tier) -> Vec<String> {
    let mut b = WalkBuilder::new(repo_root.join(dir));
    b.hidden(false).follow_links(false).require_git(false);
    if tier == Tier::Local {
        b.standard_filters(false);
    }
    let mut out: Vec<String> = b
        .build()
        .flatten()
        .filter(|e| e.file_type().is_some_and(|t| t.is_file()))
        .filter_map(|e| crate::paths::rel_of(repo_root, e.path()))
        .collect();
    for p in crate::corpus::overlay::unsaved_under(&repo_root.join(dir)) {
        if let Some(r) = crate::paths::rel_of(repo_root, &p) {
            out.push(r);
        }
    }
    out.sort();
    out
}
