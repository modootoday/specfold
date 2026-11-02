//! F1: documents for a public repository kept in a private directory outside it. A user-level
//! file maps the repository to that directory; nothing is written inside the public tree.
//!
//! ```toml
//! # ~/.config/specfold/roots.toml (or $SPECFOLD_ROOTS)
//! [[root]]
//! repo = "~/src/public-lib"
//! spec = "~/notes/public-lib-spec"
//! ```

use super::paths;
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct Roots {
    #[serde(default)]
    root: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    repo: String,
    spec: String,
}

/// The user-level mapping file: `$SPECFOLD_ROOTS`, else `$XDG_CONFIG_HOME/specfold/roots.toml`,
/// else `~/.config/specfold/roots.toml`.
pub fn file() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("SPECFOLD_ROOTS") {
        return Some(PathBuf::from(p));
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| paths::home_dir().map(|h| h.join(".config")))?;
    Some(base.join("specfold/roots.toml"))
}

/// The private root mapped to the repository at `repo`, when the mapping file names it.
pub fn mapped(repo: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(file()?).ok()?;
    let roots: Roots = toml::from_str(&text).ok()?;
    let canon = |p: &Path| p.canonicalize().ok();
    let me = canon(repo)?;
    roots.root.into_iter().find_map(|e| {
        let r = paths::expand(Path::new("/"), &e.repo);
        (canon(&r)? == me).then(|| paths::expand(Path::new("/"), &e.spec))
    })
}
