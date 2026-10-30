//! The graph derived from the corpus: lookup tables and typed edges between documents.

use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// Relation kinds that point at other documents (`governs` points at code paths instead).
pub const DOC_RELATIONS: &[&str] =
    &["supersedes", "implements", "evidence", "depends_on", "translation_of", "exception_of"];
pub const MENTIONS: &str = "mentions";

#[derive(Debug, Clone, Serialize)]
pub struct Edge {
    pub from: usize,
    pub to: usize,
    pub kind: String,
    pub anchor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    Doc {
        idx: usize,
        anchor: Option<String>,
        via_alias: bool,
    },
    Ambiguous(Vec<usize>),
    /// The document exists but the anchor does not.
    MissingAnchor {
        idx: usize,
        anchor: String,
    },
    Missing,
}

#[derive(Debug, Default)]
pub struct Graph {
    pub by_id: HashMap<String, usize>,
    pub by_path: HashMap<String, usize>,
    pub by_alias: HashMap<String, Vec<usize>>,
    /// Slug ids (`plan:root/payment-rollout`), resolved like aliases.
    pub by_slug: HashMap<String, Vec<usize>>,
    pub anchors: Vec<HashSet<String>>,
    pub edges: Vec<Edge>,
    pub outgoing: Vec<Vec<usize>>,
    pub incoming: Vec<Vec<usize>>,
}
