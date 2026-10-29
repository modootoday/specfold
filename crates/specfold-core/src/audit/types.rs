//! Audit items: things that pass every check yet need attention.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// Governed code changed long after the document was last updated.
    Stale,
    /// Active, governs nothing, cited by nothing.
    Orphan,
    /// An owner with code but no active source of truth.
    Gap,
    /// An active plan with no recent update, or an old draft.
    Stalled,
    /// A plan whose tasks and status disagree: all done but active, or archived with open tasks.
    Finished,
    /// Markers of unfinished work inside a document.
    OpenItem,
    /// Leftover compatibility: alias citations, skipped rules.
    Debt,
    /// Reported by an audit plugin.
    Plugin,
}

#[derive(Debug, Clone, Serialize)]
pub struct Item {
    pub kind: Kind,
    /// The document or owner the item is about.
    pub subject: String,
    pub path: String,
    pub line: usize,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Diagnosis {
    pub items: Vec<Item>,
    pub docs: usize,
    pub owners: usize,
}
