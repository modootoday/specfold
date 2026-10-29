//! How checks judge the repository: retired layouts, the alias table, the baseline and severity,
//! and citations outside documents.

use crate::config::defaults;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ForbidSection {
    /// Globs that must not exist anywhere in the repository.
    #[serde(default)]
    pub paths: Vec<String>,
    /// Regexes that must not appear inside spec documents (old id schemes, for example).
    #[serde(default)]
    pub patterns: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AliasSection {
    /// A frozen `old<TAB>new-id` table written by `spec import`.
    pub file: Option<String>,
}

/// How `spec check` judges an existing pile.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CheckSection {
    /// A committed file of known findings (`spec check --write-baseline`); only new ones fail.
    pub baseline: Option<String>,
    /// Paths where every finding counts, baseline or not.
    #[serde(default)]
    pub strict: Vec<String>,
    /// Rule id → "error", "warn" or "off".
    #[serde(default)]
    pub severity: BTreeMap<String, String>,
    /// Frontmatter keys every type may carry beyond the common set (rule S1); `prefix*` allows a
    /// family such as `review_*`.
    #[serde(default)]
    pub extra_keys: Vec<String>,
    /// Markdown files outside documents whose spec:count markers are checked (rule N1), as paths
    /// or globs such as `README.md` and `docs/**/*.md`; off when empty.
    #[serde(default)]
    pub count_paths: Vec<String>,
}

impl CheckSection {
    /// Whether `key` is allowed on every type by `extra_keys`.
    pub fn extra_key(&self, key: &str) -> bool {
        self.extra_keys.iter().any(|p| match p.strip_suffix('*') {
            Some(prefix) => key.starts_with(prefix),
            None => p == key,
        })
    }
}

/// Files outside spec documents whose citations of documents are checked (rule L6): code
/// comments, READMEs, agent instruction files.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CitesSection {
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
    /// Regexes for lines that record history ("moved to …"); citations on them are not judged.
    #[serde(default = "defaults::history_patterns")]
    pub history: Vec<String>,
}

impl Default for CitesSection {
    fn default() -> Self {
        CitesSection { paths: vec![], exclude: vec![], history: defaults::history_patterns() }
    }
}
