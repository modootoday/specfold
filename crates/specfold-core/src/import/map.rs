//! The import map: how files from another layout become typed documents. A project writes it
//! once for its migration; nothing in it is specific to specfold's own layout.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImportMap {
    #[serde(default)]
    pub sources: Vec<SourceRule>,
    /// Old status → new status, for every source.
    #[serde(default)]
    pub status: BTreeMap<String, String>,
    /// Old frontmatter key → new key, or "-" to drop it. `rel.<kind>` moves a value into rel.
    /// A trailing `?` (`check = "verify?"`) sets the key only when no other old key filled it.
    #[serde(default)]
    pub keys: BTreeMap<String, String>,
    /// Also record each source's bare name (file name without `.md`, or directory name) as an
    /// alias, for layouts whose documents referred to each other by name.
    #[serde(default)]
    pub stem_aliases: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRule {
    /// Files to import; a pattern ending in `/` matches directories, imported as bundles.
    pub glob: String,
    #[serde(default)]
    pub exclude: Vec<String>,
    /// The document type. Required unless `redirect` is set.
    #[serde(rename = "type")]
    pub ty: Option<String>,
    /// `auto` (from governs, then from the code paths the text mentions) or a fixed owner id.
    #[serde(default = "auto")]
    pub owner: String,
    /// How the key is chosen for timestamp types: `name` (from `name`'s `key` group),
    /// `first-commit` (the file's first commit time) or `created` (frontmatter created date).
    #[serde(default = "first_commit")]
    pub key: String,
    /// A regex over the file or directory name with optional named groups `key`, `slug`, `alias`.
    pub name: Option<String>,
    /// Status for documents without one.
    pub default_status: Option<String>,
    /// Status overrides for this source only.
    #[serde(default)]
    pub status: BTreeMap<String, String>,
    /// The file is a "moved" stub: a regex with a `target` group naming the new path. Stubs become
    /// alias rows and are removed.
    pub redirect: Option<String>,
    /// Extra frontmatter fields whose values become aliases (old ids).
    #[serde(default)]
    pub alias_fields: Vec<String>,
    /// For files: also take the sibling directory `<stem>/` as the bundle.
    #[serde(default)]
    pub sibling_dir: bool,
}

fn auto() -> String {
    "auto".into()
}

fn first_commit() -> String {
    "first-commit".into()
}

impl ImportMap {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("invalid import map {}", path.display()))
    }
}
