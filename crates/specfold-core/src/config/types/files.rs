//! Attachments: the size kept in the repository and the store for larger ones.

use crate::config::defaults;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FilesSection {
    /// Larger attachments go to the store.
    #[serde(default = "defaults::inline_max_bytes")]
    pub inline_max_bytes: u64,
    /// Repository attachments up to this size are read by the secrets check (U2); larger and
    /// binary files are not.
    #[serde(default = "defaults::scan_max_bytes")]
    pub scan_max_bytes: u64,
    pub store: Option<StoreDef>,
}

impl Default for FilesSection {
    fn default() -> Self {
        FilesSection {
            inline_max_bytes: defaults::inline_max_bytes(),
            scan_max_bytes: defaults::scan_max_bytes(),
            store: None,
        }
    }
}

/// Where attachments too large for the repository live. specfold speaks no network protocol
/// itself: a `dir` store is a plain directory, a `command` store runs the tool you already trust
/// for your bucket (rclone, aws, gsutil, wrangler …).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoreDef {
    /// `dir` or `command`.
    pub kind: String,
    /// For `dir`: the directory, `~` expanded.
    pub path: Option<String>,
    /// For `command`: argv templates. `{src}`/`{dst}` are local paths, `{key}` is the object key.
    pub put: Option<Vec<String>>,
    pub get: Option<Vec<String>>,
    pub exists: Option<Vec<String>>,
}
