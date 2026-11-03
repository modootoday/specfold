//! The submodules of the repository, read from one place: the gitlinks the index records first,
//! then any `.gitmodules` entry the index does not hold (a checkout without git, a pending add).

use crate::config::defaults::CONFIG_FILE;
use std::path::Path;

/// Repository-relative paths of every submodule, gitlinks first.
pub fn submodules(root: &Path) -> Vec<String> {
    let mut out = crate::workspace::vcs::gitlinks(root);
    for p in declared(root) {
        if !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

/// A1: the submodules that carry their own spec.toml, as (remote name, path); the last path
/// segment names each one.
pub fn submodule_remotes(root: &Path) -> Vec<(String, String)> {
    submodules(root)
        .into_iter()
        .filter_map(|path| {
            let name = path.rsplit('/').next().unwrap_or(&path).to_string();
            (!name.is_empty() && root.join(&path).join(CONFIG_FILE).is_file()).then_some((name, path))
        })
        .collect()
}

/// The `path` entries of `.gitmodules`.
fn declared(root: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(root.join(".gitmodules")) else { return vec![] };
    text.lines()
        .filter_map(|line| line.split_once('='))
        .filter(|(k, _)| k.trim() == "path")
        .map(|(_, v)| v.trim().trim_matches('"').trim_end_matches('/').to_string())
        .filter(|p| !p.is_empty())
        .collect()
}
