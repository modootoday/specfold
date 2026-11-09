//! Limits a project keeps on its files: size budgets (B1) and generated files named by a
//! manifest (G2).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// `[budgets]`: bytes and lines per path glob, here or in another file that owns the numbers.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetsSection {
    /// `<file>#<table>`: a TOML or JSON file, relative to the spec home, and the dotted key of
    /// the table that holds the limits there (the whole file without `#`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// Limits written here, glob → `{ bytes, lines }`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub paths: BTreeMap<String, BudgetLimit>,
}

/// One glob's limits; at least one is set.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetLimit {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines: Option<u64>,
}

/// `[generated]`: files another tool writes, read from that tool's manifest.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedSection {
    /// `<file>[#<key>]`: a JSON or TOML manifest whose entries carry `path` and optionally
    /// `source` or `sources`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest: Option<String>,
}
