//! Thresholds `spec audit` diagnoses against.

use crate::config::defaults;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AuditSection {
    /// Governed code changed this many days after the document's `updated`.
    #[serde(default = "defaults::stale_days")]
    pub stale_days: i64,
    /// An active plan with no update for this many days.
    #[serde(default = "defaults::stalled_days")]
    pub stalled_days: i64,
    /// A draft older than this.
    #[serde(default = "defaults::draft_days")]
    pub draft_days: i64,
    /// Text that marks unfinished work inside documents.
    #[serde(default = "defaults::open_item_patterns")]
    pub open_item_patterns: Vec<String>,
}

impl Default for AuditSection {
    fn default() -> Self {
        AuditSection {
            stale_days: defaults::stale_days(),
            stalled_days: defaults::stalled_days(),
            draft_days: defaults::draft_days(),
            open_item_patterns: defaults::open_item_patterns(),
        }
    }
}
