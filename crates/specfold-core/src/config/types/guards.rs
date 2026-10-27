//! A guard's definition: what it reads, what it denies, how it proves itself and its baseline.

use crate::config::defaults;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GuardDef {
    pub name: String,
    #[serde(default)]
    pub about: String,
    /// Patterns the guard looks at; empty means the whole repository.
    #[serde(default)]
    pub paths: Vec<String>,
    /// Also look at every published path ([public] paths, public submodules, detectors); one that
    /// is missing is a hit, since it is still published and nothing would read it.
    #[serde(default)]
    pub public: bool,
    #[serde(default)]
    pub exclude: Vec<String>,
    /// Default range when none is given: `staged`, `push` or `all`.
    #[serde(default = "defaults::guard_range")]
    pub range: String,
    /// Case-insensitive terms, inline.
    #[serde(default)]
    pub deny_terms: Vec<String>,
    /// A file of terms, one per line, `#` comments; may live outside the repository.
    pub deny_terms_file: Option<String>,
    /// An environment variable holding terms separated by newlines or commas.
    pub deny_terms_env: Option<String>,
    /// Patterns over file contents; lookaround and backreferences are allowed.
    #[serde(default)]
    pub deny_regex: Vec<RegexRule>,
    /// Patterns over the repository-relative path of every file read.
    #[serde(default)]
    pub deny_path_regex: Vec<RegexRule>,
    #[serde(default)]
    pub deny_files: Vec<String>,
    pub max_file_bytes: Option<u64>,
    /// Built-in secret patterns (private keys, cloud and service tokens).
    #[serde(default)]
    pub secrets: bool,
    /// Also scan commit messages and author names in the range.
    #[serde(default)]
    pub commit_messages: bool,
    #[serde(default = "defaults::guard_level")]
    pub level: String,
    /// Substrings that make a hit acceptable when the matched text contains one (case-insensitive).
    #[serde(default)]
    pub allow: Vec<String>,
    /// Lines the guard must flag and lines it must pass; checked before every run, so a pattern
    /// that stopped matching fails loudly instead of passing everything.
    #[serde(default)]
    pub fixtures: GuardFixtures,
    /// Read gzip-compressed files (any file starting with the gzip header, .gz or .tgz) as text.
    #[serde(default)]
    pub decompress: bool,
    /// Read binary files as Latin-1 text instead of skipping them; a published binary is published.
    #[serde(default)]
    pub read_binary: bool,
    /// A whole-tree run that scanned no file in the guard's paths fails.
    #[serde(default = "defaults::yes")]
    pub fail_if_empty: bool,
    /// A committed spec.baseline.v1 file of known hits (`spec guard <name> --write-baseline`).
    pub baseline: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GuardFixtures {
    #[serde(default)]
    pub must_hit: Vec<String>,
    #[serde(default)]
    pub must_pass: Vec<String>,
}

/// A denied regex: a plain pattern, or a table with its own options and fixtures.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum RegexRule {
    Plain(String),
    Full(RegexSpec),
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RegexSpec {
    pub re: String,
    /// The name findings use instead of the pattern.
    pub label: Option<String>,
    /// Ignore matches inside a URL.
    #[serde(default)]
    pub not_in_url: bool,
    /// Ignore matches inside a regular-expression literal of a JavaScript or TypeScript file,
    /// where a rule has to spell what it recognises.
    #[serde(default)]
    pub not_in_regex_literal: bool,
    /// Paths where matches inside Markdown or HTML code (fences, inline code, <code>) are
    /// examples, not prose.
    #[serde(default)]
    pub code_exempt: Vec<String>,
    /// Paths this pattern does not apply to.
    #[serde(default)]
    pub exempt: Vec<String>,
    #[serde(default)]
    pub must_hit: Vec<String>,
    #[serde(default)]
    pub must_pass: Vec<String>,
}

impl RegexRule {
    pub fn spec(&self) -> RegexSpec {
        match self {
            RegexRule::Plain(re) => RegexSpec { re: re.clone(), ..Default::default() },
            RegexRule::Full(s) => s.clone(),
        }
    }
}
