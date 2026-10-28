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
}

#[derive(Debug, Clone, Default)]
pub struct LoadOptions {
    /// Also read the git-ignored local directory.
    pub include_local: bool,
}
