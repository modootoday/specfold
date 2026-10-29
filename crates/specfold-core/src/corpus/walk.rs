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
        .filter_map(|e| e.path().strip_prefix(repo_root).ok().map(|p| p.to_string_lossy().replace('\\', "/")))
        .collect();
    for p in super::overlay::unsaved_under(&repo_root.join(dir)) {
        if let Ok(r) = p.strip_prefix(repo_root) {
            out.push(r.to_string_lossy().replace('\\', "/"));
        }
    }
    out.sort();
    out
}
