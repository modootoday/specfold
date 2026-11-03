//! The walks over the repository's files. Every walk from the repository root starts here, so
//! the project boundary is applied in one place.

use crate::config::Config;
use ignore::WalkBuilder;
use std::path::Path;

/// This project's files from `start`: hidden files included, ignore files honoured, and every
/// nested project (`[owners] nested`) left out. Callers may relax the ignore rules further.
pub fn walk(cfg: &Config, start: &Path) -> WalkBuilder {
    let mut b = WalkBuilder::new(start);
    b.hidden(false).filter_entry(outside_nested(cfg));
    b
}

/// Every file under `starts` across project boundaries, skipping only `.git`: for guards, which
/// look at every published byte, and for imports, which read another layout's tree.
pub fn walk_across(starts: &[std::path::PathBuf]) -> Option<WalkBuilder> {
    let (first, rest) = starts.split_first()?;
    let mut b = WalkBuilder::new(first);
    for d in rest {
        b.add(d);
    }
    b.hidden(false).require_git(false).filter_entry(|e| e.file_name() != ".git");
    Some(b)
}

/// A filter for walks from the repository root that keeps out every nested project.
fn outside_nested(cfg: &Config) -> impl Fn(&ignore::DirEntry) -> bool + Send + Sync + 'static {
    let root = cfg.spec_home.clone();
    let owners = cfg.raw.owners.clone();
    move |e| crate::paths::rel_of(&root, e.path()).is_none_or(|r| r.is_empty() || !owners.in_nested(&r))
}
