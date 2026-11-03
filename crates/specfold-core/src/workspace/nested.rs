//! Independent projects mounted inside the repository (`[owners] nested`). They keep their own
//! spec.toml, documents and CI, so this repository's checks neither read nor judge them.

use crate::config::Config;

/// A filter for directory walks from the repository root that keeps out every nested project.
pub fn outside(cfg: &Config) -> impl Fn(&ignore::DirEntry) -> bool + Send + Sync + 'static {
    let root = cfg.root.clone();
    let owners = cfg.raw.owners.clone();
    move |e| crate::paths::rel_of(&root, e.path()).is_none_or(|r| r.is_empty() || !owners.in_nested(&r))
}

/// The nested projects that carry a spec.toml, as (remote name, path): the last path segment
/// names each one, so `<name>:<id>` reads its documents.
pub fn remotes(root: &std::path::Path, owners: &crate::config::types::OwnersSection) -> Vec<(String, String)> {
    owners
        .nested_dirs()
        .filter(|p| root.join(p).join(crate::config::defaults::CONFIG_FILE).is_file())
        .map(|p| (p.rsplit('/').next().unwrap_or(p).to_string(), p.to_string()))
        .collect()
}
