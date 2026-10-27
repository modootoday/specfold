//! How `spec verify` runs the commands documents name.

use crate::config::defaults;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerifySection {
    /// Default deadline per command; a document's `verify_timeout_ms` overrides it.
    #[serde(default = "defaults::verify_timeout_ms")]
    pub timeout_ms: u64,
    /// Commands run at once; 0 means the number of CPUs, at most 8.
    #[serde(default)]
    pub jobs: usize,
    /// Regexes for heavy commands (test suites, type checks) that run one at a time in their own lane.
    #[serde(default)]
    pub serial: Vec<String>,
    /// Paths whose change runs every verify command (lockfiles, shared manifests, the checks themselves).
    #[serde(default)]
    pub full_run_on: Vec<String>,
}

impl Default for VerifySection {
    fn default() -> Self {
        VerifySection { timeout_ms: defaults::verify_timeout_ms(), jobs: 0, serial: vec![], full_run_on: vec![] }
    }
}
