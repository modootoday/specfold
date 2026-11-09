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
    /// A1: register each submodule that has its own spec.toml as a read-only remote named after
    /// the submodule (its last path segment). Off by default; an explicit `[remotes]` entry wins.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub submodule_remotes: bool,
}

impl Default for SpecSection {
    fn default() -> Self {
        SpecSection { schema: defaults::schema(), dir: defaults::spec_dir(), submodule_remotes: false }
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
    /// A directory or a file, outside the repository or in it, `~` expanded.
    pub path: String,
    /// Only `cite-check` exists: report citations that no longer resolve.
    #[serde(default = "defaults::cite_check")]
    pub mode: String,
    /// Rule V3: every backticked repository path in these files exists (a path inside a gitlink
    /// counts when the pinned commit holds it). Off unless set.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub check_paths: bool,
    /// Rule V3: every document id in these files resolves, bare or in `[[…]]`. Off unless set.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ids: bool,
}

/// Another repository's documents, read-only: `<name>:<id>` in references and relations names a
/// document there. The directory is a checkout already on disk (sibling, submodule, CI step).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteRoot {
    /// The other repository's root (its spec.toml or git root), relative to this one or `~`.
    pub path: String,
    /// More places it may be checked out (a sibling here, a submodule in CI); the first directory
    /// on disk among `path` and these is used.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
    /// Registered from `.gitmodules` by `[spec] submodule_remotes` or from `[owners] nested`, not
    /// written in spec.toml.
    #[serde(skip)]
    pub auto: bool,
}
