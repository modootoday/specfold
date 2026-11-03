//! Other projects inside this repository: submodules of their own, nested projects and remotes
//! checked out here. Governs into them are another project's.

use crate::config::Config;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// A governed path that belongs to another project: written `<remote>:<path>`, or inside a
/// remote checked out within this repository, or inside a submodule no owner kind claims. P1 and
/// L5 leave it out; D1 still matches it.
pub fn is_external(cfg: &Config, governs: &str) -> bool {
    if crate::remotes::split(cfg, governs).is_some() {
        return true;
    }
    let g = governs.trim_start_matches("./").trim_end_matches('/');
    let under = |p: &str| {
        let p = p.trim_start_matches("./").trim_end_matches('/');
        !p.is_empty() && crate::paths::is_under(g, p)
    };
    inner_projects(cfg).iter().any(|p| under(p))
}

/// A1: the auto-registered submodule remote whose directory holds `rel`, and the path inside it.
pub fn submodule_at(cfg: &Config, rel: &str) -> Option<(String, String)> {
    cfg.raw.remotes.iter().filter(|(_, r)| r.auto).find_map(|(name, r)| {
        let p = r.path.trim_end_matches('/');
        let inner = if rel == p { "" } else { rel.strip_prefix(p)?.strip_prefix('/')? };
        Some((name.clone(), inner.to_string()))
    })
}

/// Whether the submodule at `path` is a project of its own rather than code of an owner here. A
/// submodule an owner kind claims is that owner's code, with or without its own spec.toml (a
/// product split into its own repository); any other submodule is vendored or another project.
fn own_project(owners: &crate::workspace::Owners, path: &str) -> bool {
    owners.by_path(path).is_none()
}

/// Projects inside this repository: submodules of their own and remotes checked out here, once
/// per root.
fn inner_projects(cfg: &Config) -> Vec<String> {
    static C: OnceLock<Mutex<HashMap<PathBuf, Vec<String>>>> = OnceLock::new();
    let mut c = C.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    c.entry(cfg.root.clone())
        .or_insert_with(|| {
            let owners = crate::workspace::Owners::discover(cfg);
            let mut v: Vec<String> =
                super::submodules(&cfg.root).into_iter().filter(|p| own_project(&owners, p)).collect();
            v.extend(cfg.raw.owners.nested_dirs().map(String::from));
            // Submodules registered as remotes by `submodule_remotes` were judged above.
            for r in cfg.raw.remotes.values().filter(|r| !r.auto) {
                for p in std::iter::once(&r.path).chain(&r.paths) {
                    if !p.starts_with("..") && !p.starts_with('/') && !p.starts_with('~') {
                        v.push(p.clone());
                    }
                }
            }
            v
        })
        .clone()
}
