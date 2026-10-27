//! Search requests and hits.

use serde::Serialize;

#[derive(Debug, Clone, Default)]
pub struct Query {
    /// Words that must all appear in the id, title, slug, aliases or tags (case-insensitive).
    pub words: Vec<String>,
    pub types: Vec<String>,
    pub owners: Vec<String>,
    pub statuses: Vec<String>,
    pub tags: Vec<String>,
    /// Include superseded, archived and rejected documents.
    pub all: bool,
    /// Also search the body text.
    pub text: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Hit {
    pub id: String,
    pub title: String,
    pub status: String,
    pub path: String,
    /// Higher is better: exact id or alias, then title words, then body.
    pub score: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excerpt: Option<String>,
}
