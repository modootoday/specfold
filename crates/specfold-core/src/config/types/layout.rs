//! Where documents live: the spec directory name, the local tier and outside roots.

use crate::config::defaults;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpecSection {
    #[serde(default = "defaults::schema")]
    pub schema: u32,
    /// Directory name that holds an owner's documents.
    #[serde(default = "defaults::spec_dir")]
    pub dir: String,
}

impl Default for SpecSection {
    fn default() -> Self {
        SpecSection { schema: defaults::schema(), dir: defaults::spec_dir() }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LocalSection {
    #[serde(default = "defaults::local_dir")]
    pub path: String,
}

impl Default for LocalSection {
    fn default() -> Self {
        LocalSection { path: defaults::local_dir() }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalRoot {
    /// A directory outside the repository, `~` expanded.
    pub path: String,
    /// Only `cite-check` exists: report citations that no longer resolve.
    #[serde(default = "defaults::cite_check")]
    pub mode: String,
}

/// Another repository's documents, read-only: `<name>:<id>` in references and relations names a
/// document there. The directory is a checkout already on disk (sibling, submodule, CI step).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteRoot {
    /// The other repository's root (its spec.toml or git root), relative to this one or `~`.
    pub path: String,
}
