//! F1: documents for a public repository kept in a private directory outside it. A user-level
//! file maps the repository to that directory; nothing is written inside the public tree.
//!
//! It also names other repositories by id prefix (F2), read like `[remotes]`.
//!
//! ```toml
//! # ~/.config/specfold/roots.toml (or $SPECFOLD_ROOTS)
//! [[root]]
//! repo = "~/src/public-lib"
//! spec = "~/notes/public-lib-spec"
//! [prefix]
//! lib = "~/src/other-lib"   # spec show lib:<id>
//! ```

use super::paths;
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct Roots {
    #[serde(default)]
    root: Vec<Entry>,
    /// F2: id prefix → repository root, read-only (`[prefix] lib = "~/src/lib"`).
    #[serde(default)]
    prefix: std::collections::BTreeMap<String, String>,
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

fn read() -> Option<Roots> {
    toml::from_str(&std::fs::read_to_string(file()?).ok()?).ok()
}

/// F2: the id prefixes of the user-level file, as (prefix, absolute root).
pub fn prefixes() -> Vec<(String, PathBuf)> {
    read()
        .map(|r| r.prefix.into_iter().map(|(k, v)| (k, paths::expand(Path::new("/"), &v))).collect())
        .unwrap_or_default()
}

/// The private root mapped to the repository at `repo`, when the mapping file names it.
pub fn mapped(repo: &Path) -> Option<PathBuf> {
    let roots = read()?;
    let canon = |p: &Path| p.canonicalize().ok();
    let me = canon(repo)?;
    roots.root.into_iter().find_map(|e| {
        let r = paths::expand(Path::new("/"), &e.repo);
        (canon(&r)? == me).then(|| paths::expand(Path::new("/"), &e.spec))
    })
}
