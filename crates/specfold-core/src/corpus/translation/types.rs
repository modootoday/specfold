//! What translations are described with: source parts, section changes, layout pairs and states.

use serde::Serialize;
use std::collections::BTreeMap;

pub const RELATION: &str = "translation_of";
pub const KEY: &str = "source_rev";

/// One file of a source: `label` is empty for the main file and the chapter path otherwise.
#[derive(Debug, Clone)]
pub struct Part {
    pub label: String,
    pub body: String,
}

/// How one section of a source differs from the revision a translation was made from.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SectionChange {
    /// The heading text, prefixed with the chapter file for chapters; `(top)` before the first
    /// heading.
    pub section: String,
    /// `changed`, `added` or `removed`.
    pub change: String,
    /// Paragraphs that differ (for `changed`) or that the section has (otherwise).
    pub paragraphs: usize,
}

/// A pair the layout infers for a document in a translation position.
#[derive(Debug, Clone)]
pub struct Inferred {
    pub lang: String,
    /// The source, when exactly one document is in the source position.
    pub source: Option<usize>,
    /// Every document in the source position (more than one is ambiguous).
    pub candidates: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// source_rev matches the source.
    Current,
    /// The source changed after source_rev.
    Behind,
    /// No source_rev recorded.
    Unrecorded,
    /// rel.translation_of does not name a document.
    NoSource,
}

#[derive(Debug, Clone, Serialize)]
pub struct Translation {
    pub id: String,
    pub path: String,
    pub lang: Option<String>,
    pub source: String,
    pub state: State,
    /// For a translation behind its source: the sections changed since source_rev, when git has
    /// that revision.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<SectionChange>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Report {
    pub translations: Vec<Translation>,
    /// Per language: sources with no translation in it.
    pub missing: BTreeMap<String, Vec<String>>,
    pub sources: usize,
}
