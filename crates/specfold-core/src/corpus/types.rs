//! The loaded corpus: every document, every path that does not belong, and the alias table.

use crate::config::Config;
use crate::model::{Doc, Tier};
use crate::workspace::Owners;
use serde::Serialize;
use std::collections::BTreeMap;

/// A spec directory that holds documents.
#[derive(Debug, Clone, Serialize)]
pub struct SpecRoot {
    pub owner: String,
    /// Relative to the repository root.
    pub dir: String,
    pub tier: Tier,
    /// The owner is not imported yet: its typed documents are read, its old layout is not.
    pub pending: bool,
    /// The one type whose documents this directory holds directly (`[types.<name>] dir`); none
    /// for a spec directory, which holds a subdirectory per type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ty: Option<String>,
    /// A type directory the working spec.toml dropped while HEAD's still has it: its documents
    /// are read only so that S3 can name them and `spec mv` can take them over.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub former: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Stray {
    pub path: String,
    pub reason: String,
}

/// Old identifiers and paths from before an import, mapped to current ids.
#[derive(Debug, Clone, Default)]
pub struct AliasTable {
    pub file: Option<String>,
    pub map: BTreeMap<String, Vec<String>>,
}

#[derive(Debug)]
pub struct Corpus {
    pub cfg: Config,
    pub owners: Owners,
    pub roots: Vec<SpecRoot>,
    pub docs: Vec<Doc>,
    pub strays: Vec<Stray>,
    pub aliases: AliasTable,
    /// Markdown files in committed spec directories that git does not track, left out because
    /// the configuration reads tracked files only (`Config.tracked`); empty otherwise.
    pub untracked: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct LoadOptions {
    /// Also read the git-ignored local directory.
    pub include_local: bool,
}
