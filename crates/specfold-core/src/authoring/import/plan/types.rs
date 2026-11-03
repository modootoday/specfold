//! What an import plan holds: the review rows and the changes that apply them.

use crate::authoring::Plan;
use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
pub struct Row {
    pub source: String,
    /// `doc` or `redirect`.
    pub kind: String,
    pub id: String,
    pub path: String,
    pub owner_reason: String,
    pub status_from: String,
    pub status_to: String,
    pub aliases: Vec<String>,
    pub unresolved: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// What a status pattern named as the replacing record, resolved after every row is built.
    #[serde(skip)]
    pub replaced_by: Option<String>,
    /// The old Markdown file, whose relative links are rewritten to the new paths.
    #[serde(skip)]
    pub old_main: String,
}

#[derive(Debug, Default, Serialize)]
pub struct ImportPlan {
    pub rows: Vec<Row>,
    #[serde(skip)]
    pub plan: Plan,
    pub alias_rows: Vec<(String, String)>,
    /// Attachments over the repository limit: (bundle path, sha256) to upload to the store.
    pub uploads: Vec<(String, String)>,
}
