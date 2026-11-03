//! A path inside a public repository mapped to a private spec root (roots.toml), named the way
//! the spec root names it: below the `[public]` path the repository is a copy of.

use super::{Config, private_root};
use crate::workspace::vcs;
use std::path::{Path, PathBuf};

/// Where a path outside the spec root lands.
#[derive(Debug, PartialEq, Eq)]
pub enum ClonePath {
    /// Not inside a repository mapped to this spec root.
    Outside,
    /// The spec-root-relative path, below the public path the repository is a copy of.
    Mapped(String),
    /// Inside the mapped repository, but no public path of the spec root is named for it.
    Unmapped {
        /// The mapped repository.
        clone: PathBuf,
        /// The path relative to that repository.
        rel: String,
    },
}

/// Locates `abs` (absolute, symbolic links resolved) when it lies in a repository that the
/// user-level roots.toml maps to `cfg.spec_home`.
pub fn locate(cfg: &Config, abs: &Path) -> ClonePath {
    match clone_of(cfg, abs) {
        Some(clone) => locate_in(cfg, &clone, abs),
        None => ClonePath::Outside,
    }
}

/// The repository holding `abs` (its canonical root) when roots.toml maps it to `cfg.spec_home`.
pub fn clone_of(cfg: &Config, abs: &Path) -> Option<PathBuf> {
    let clone = existing_ancestor(abs).and_then(|d| vcs::git_root(&d)).and_then(|c| c.canonicalize().ok())?;
    let m = private_root::mapping(&clone)?;
    (m.spec.canonicalize().ok() == cfg.spec_home.canonicalize().ok()).then_some(clone)
}

/// `abs` inside `clone`, a repository [`clone_of`] found, named the way the spec root names it.
pub fn locate_in(cfg: &Config, clone: &Path, abs: &Path) -> ClonePath {
    let Some(m) = private_root::mapping(clone) else { return ClonePath::Outside };
    let Some(rel) = crate::paths::rel_of(clone, abs) else { return ClonePath::Outside };
    let clone = clone.to_path_buf();
    match public_path(cfg, &clone, m.public.as_deref()) {
        Some(p) if rel.is_empty() => ClonePath::Mapped(p),
        Some(p) => ClonePath::Mapped(format!("{p}/{rel}")),
        None => ClonePath::Unmapped { clone, rel },
    }
}

/// The public path the repository is a copy of: the entry's `public` key, the only public path,
/// or the one whose last component is the repository's directory name.
fn public_path(cfg: &Config, clone: &Path, named: Option<&str>) -> Option<String> {
    let clean = |p: &str| p.trim_start_matches("./").trim_end_matches('/').to_string();
    if let Some(p) = named {
        return Some(clean(p));
    }
    let public = &cfg.raw.public;
    let mut all: Vec<String> = public
        .paths
        .iter()
        .chain(public.companion.keys())
        .map(|p| clean(p))
        .filter(|p| !p.is_empty() && !p.contains(['*', '?', '[', '{']))
        .collect();
    all.sort();
    all.dedup();
    if let [only] = all.as_slice() {
        return Some(only.clone());
    }
    let name = clone.file_name()?.to_str()?;
    let same: Vec<&String> = all.iter().filter(|p| p.rsplit('/').next() == Some(name)).collect();
    match same.as_slice() {
        [one] => Some((*one).clone()),
        _ => None,
    }
}

fn existing_ancestor(abs: &Path) -> Option<PathBuf> {
    abs.ancestors().find(|a| a.is_dir()).map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::RawConfig;

    fn cfg(paths: &[&str]) -> Config {
        let mut raw = RawConfig::default();
        raw.public.paths = paths.iter().map(|p| p.to_string()).collect();
        Config::from_raw(PathBuf::from("/r"), raw, false)
    }

    #[test]
    fn public_path_takes_the_only_one_or_the_one_named_like_the_clone() {
        let clone = Path::new("/w/cli");
        assert_eq!(public_path(&cfg(&["packages/tool"]), clone, None).as_deref(), Some("packages/tool"));
        assert_eq!(public_path(&cfg(&["libs/auth", "packages/cli/"]), clone, None).as_deref(), Some("packages/cli"));
        assert_eq!(public_path(&cfg(&["libs/auth", "libs/shop"]), clone, None), None);
        assert_eq!(public_path(&cfg(&[]), clone, None), None);
        assert_eq!(
            public_path(&cfg(&["libs/auth", "libs/shop"]), clone, Some("libs/shop")).as_deref(),
            Some("libs/shop")
        );
    }
}
