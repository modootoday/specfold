//! What the repository publishes, and where documents about published paths go instead.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PublicSection {
    /// Paths published to the world. No spec document may live under them.
    #[serde(default)]
    pub paths: Vec<String>,
    /// Where documents about a public path go instead.
    #[serde(default)]
    pub companion: BTreeMap<String, String>,
    /// Also find published paths: pages, site-generators, npm-files, cargo-include.
    #[serde(default)]
    pub detect: Vec<String>,
    /// An offline manifest of submodule visibility ({"gitmodulesSha256", "submodules": {path:
    /// {"url", "visibility"}}}), written by `spec sync visibility`.
    pub submodules_manifest: Option<String>,
}
