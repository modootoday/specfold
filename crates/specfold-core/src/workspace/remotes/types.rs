//! What a remote reference names, a remote repository once loaded, and a document that names
//! another repository's document.

use crate::corpus::Corpus;
use crate::graph::Graph;
use std::path::PathBuf;

/// What `<name>:<id>` names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Remote {
    /// Not a remote reference: no configured remote of that name.
    Local,
    /// The remote is configured but its directory is not on disk.
    Absent {
        name: String,
        path: PathBuf,
    },
    /// One document: its id, type, title and absolute file path.
    Found {
        name: String,
        id: String,
        ty: String,
        title: String,
        file: PathBuf,
    },
    Missing {
        name: String,
        id: String,
    },
    MissingAnchor {
        name: String,
        id: String,
        anchor: String,
    },
    Ambiguous {
        name: String,
        ids: Vec<String>,
    },
}

/// A remote repository read from disk: its corpus and graph.
pub struct Loaded {
    pub corpus: Corpus,
    pub graph: Graph,
}

/// A document in this repository or a remote that names another repository's document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Consumer {
    /// `.` for this repository, else the remote's name.
    pub repo: String,
    pub id: String,
    pub path: String,
    /// `rel.<kind>` or `pins`.
    pub via: String,
}
