//! Lists the files under a spec directory.

use crate::config::Config;
use crate::model::Tier;

/// Repository-relative paths of every file under `dir`. Committed roots honour `.gitignore`;
/// the local root is ignored by git on purpose, so it is read as is. Under `Config.tracked`, a
/// committed root's untracked Markdown files go to `untracked` instead of the result.
pub fn files(cfg: &Config, dir: &str, tier: Tier, untracked: &mut Vec<String>) -> Vec<String> {
    let repo_root = &cfg.spec_home;
    let mut b = crate::workspace::walk_untracked(cfg, &repo_root.join(dir));
    b.follow_links(false).require_git(false);
    if tier == Tier::Local {
        b.standard_filters(false);
    }
    let mut out: Vec<String> = b
        .build()
        .flatten()
        .filter(|e| e.file_type().is_some_and(|t| t.is_file()))
        .filter_map(|e| crate::paths::rel_of(repo_root, e.path()))
        .collect();
    if tier == Tier::Committed && cfg.tracked.is_some() {
        let (kept, left): (Vec<String>, Vec<String>) = out.into_iter().partition(|p| cfg.reads(p, false));
        untracked.extend(left.into_iter().filter(|p| p.ends_with(".md")));
        out = kept;
    }
    for p in crate::corpus::overlay::unsaved_under(&repo_root.join(dir)) {
        if let Some(r) = crate::paths::rel_of(repo_root, &p) {
            out.push(r);
        }
    }
    out.sort();
    out
}
