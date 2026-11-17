//! The workspace that mounts this repository (`[owners] mount`). It sits at the repository root
//! minus the mount path and is read once, only when a target is not found here. A clone checked
//! out on its own has no such workspace, and its targets there are skipped, not reported.

use super::load::load_dir;
use super::types::{Loaded, Remote};
use crate::config::Config;
use crate::corpus::graph::Resolution;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// What the mounting workspace is for this repository.
pub enum Parent {
    /// No `[owners] mount`: targets are this repository's alone.
    NotMounted,
    /// A mount is declared but the workspace is not on disk around this checkout.
    Absent,
    Loaded(Arc<Loaded>),
}

/// The name parent lookups carry in a `Remote`.
pub const PARENT: &str = "the mounting workspace";

/// The directory the mount puts the workspace root at: `root` with the mount's components
/// removed from its end; `None` when `root` does not end with them.
pub fn root_of(root: &Path, mount: &str) -> Option<PathBuf> {
    let mut dir = root.to_path_buf();
    for part in mount.rsplit('/').filter(|p| !p.is_empty() && *p != ".") {
        if dir.file_name()?.to_str()? != part {
            return None;
        }
        dir.pop();
    }
    Some(dir)
}

/// The mounting workspace of `cfg`, loaded once per process.
pub fn parent(cfg: &Config) -> Parent {
    let Some(mount) = cfg.raw.owners.mount_dir() else { return Parent::NotMounted };
    let home = std::fs::canonicalize(&cfg.spec_home).unwrap_or_else(|_| cfg.spec_home.clone());
    let Some(dir) = root_of(&home, mount).filter(|d| d.join(crate::config::defaults::CONFIG_FILE).is_file()) else {
        return Parent::Absent;
    };
    load_dir(&dir, "workspace").map_or(Parent::Absent, Parent::Loaded)
}

/// Resolves `text` in the mounting workspace, with its aliases: `Local` without a mount,
/// `Absent` when the workspace is not on disk.
pub fn resolve_in_parent(cfg: &Config, text: &str) -> Remote {
    let name = PARENT.to_string();
    let l = match parent(cfg) {
        Parent::NotMounted => return Remote::Local,
        Parent::Absent => return Remote::Absent { name, path: PathBuf::new() },
        Parent::Loaded(l) => l,
    };
    super::query::resolve_loaded(name, &l, text)
}

/// T1: whether a document of the mounting workspace supersedes `id`: `None` without a mount,
/// `Some(true)` when one does or the workspace is absent (nothing can be said), else `Some(false)`.
pub fn superseded_in_parent(cfg: &Config, id: &str) -> Option<bool> {
    let l = match parent(cfg) {
        Parent::NotMounted => return None,
        Parent::Absent => return Some(true),
        Parent::Loaded(l) => l,
    };
    let Resolution::Doc { idx, .. } = l.graph.resolve(id) else {
        return Some(
            l.corpus.docs.iter().any(|d| d.rel().get("supersedes").is_some_and(|v| v.iter().any(|t| t == id))),
        );
    };
    let ty = &l.corpus.docs[idx].id.ty;
    Some(l.graph.edges_to(idx).any(|e| e.kind == "supersedes" && &l.corpus.docs[e.from].id.ty == ty))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_workspace_root_is_the_repository_root_without_the_mount() {
        let r = Path::new("/w/apps/shop");
        assert_eq!(root_of(r, "apps/shop"), Some(PathBuf::from("/w")));
        assert_eq!(root_of(r, "shop"), Some(PathBuf::from("/w/apps")));
        assert_eq!(root_of(r, "libs/shop"), None);
        assert_eq!(root_of(Path::new("/shop"), "apps/shop"), None);
    }
}
