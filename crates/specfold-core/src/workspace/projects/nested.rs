//! Independent projects mounted inside the repository (`[owners] nested`). They keep their own
//! spec.toml, documents and CI, so this repository's checks neither read nor judge them.

use crate::config::defaults::CONFIG_FILE;
use crate::config::types::OwnersSection;
use std::path::Path;

/// The nested projects that carry a spec.toml, as (remote name, path): the last path segment
/// names each one, so `<name>:<id>` reads its documents.
pub fn nested_remotes(root: &Path, owners: &OwnersSection) -> Vec<(String, String)> {
    owners
        .nested_dirs()
        .filter(|p| root.join(p).join(CONFIG_FILE).is_file())
        .map(|p| (p.rsplit('/').next().unwrap_or(p).to_string(), p.to_string()))
        .collect()
}
