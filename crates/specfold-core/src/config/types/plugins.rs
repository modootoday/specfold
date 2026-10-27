//! An external program that extends specfold over the `specfold.plugin.v1` JSON protocol.

use crate::config::defaults;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PluginDef {
    pub name: String,
    /// argv; relative programs are resolved against the repository root.
    pub cmd: Vec<String>,
    #[serde(default = "defaults::plugin_timeout_ms")]
    pub timeout_ms: u64,
}
