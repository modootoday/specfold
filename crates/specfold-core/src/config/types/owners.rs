//! Owner kinds: which directories own documents and what their ids are.

use crate::config::defaults;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerKind {
    /// A pattern of directories relative to the repository root, e.g. `services/*` or `libs/*/*`.
    pub pattern: String,
    /// The owner id template; `{1}`, `{2}` … are the `*` segments in order.
    pub id: String,
    /// Only directories that contain one of the manifests count.
    #[serde(default)]
    pub manifest: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OwnersSection {
    /// Id of the repository root as an owner.
    #[serde(default = "defaults::root_owner")]
    pub root: String,
    #[serde(default)]
    pub kinds: Vec<OwnerKind>,
    /// Replaces the default manifest list.
    pub manifests: Option<Vec<String>>,
    /// Directories whose code is gone; documents that only govern them belong to the root.
    #[serde(default)]
    pub retired: Vec<String>,
    /// Directories whose spec directory still holds a layout `spec import` has not brought in:
    /// typed documents there are read, files of the old layout are left unreported.
    #[serde(default)]
    pub pending: Vec<String>,
    /// Independent projects mounted here (a submodule with its own spec.toml, documents and CI):
    /// never an owner, their documents are not read, governs into them are another project's,
    /// and each one with a spec.toml is a read-only remote named after its last path segment.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nested: Vec<String>,
}

impl Default for OwnersSection {
    fn default() -> Self {
        OwnersSection {
            root: defaults::root_owner(),
            kinds: vec![],
            manifests: None,
            retired: vec![],
            pending: vec![],
            nested: vec![],
        }
    }
}

impl OwnersSection {
    /// Whether `rel` (a repository-relative path) is a nested project or inside one.
    pub fn in_nested(&self, rel: &str) -> bool {
        let rel = rel.trim_start_matches("./").trim_end_matches('/');
        self.nested_dirs().any(|n| crate::paths::is_under(rel, n))
    }

    /// The nested project directories, without leading `./` or a trailing slash.
    pub fn nested_dirs(&self) -> impl Iterator<Item = &str> {
        self.nested.iter().map(|n| n.trim_start_matches("./").trim_end_matches('/')).filter(|n| !n.is_empty())
    }
}
