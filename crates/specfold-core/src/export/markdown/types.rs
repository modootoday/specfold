//! What the markdown exporter is told and what it builds: target, options and the tree.

use serde::Serialize;
use std::collections::BTreeMap;

/// The file each export writes listing its own files, so the next one replaces only those.
pub const MANIFEST: &str = ".specfold-export.json";

/// The site or editor the tree is laid out for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Target {
    Obsidian,
    Mkdocs,
    Docusaurus,
    GithubWiki,
}

impl Target {
    pub const ALL: [&str; 4] = ["obsidian", "mkdocs", "docusaurus", "github-wiki"];

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "obsidian" => Target::Obsidian,
            "mkdocs" => Target::Mkdocs,
            "docusaurus" => Target::Docusaurus,
            "github-wiki" => Target::GithubWiki,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Target::Obsidian => "obsidian",
            Target::Mkdocs => "mkdocs",
            Target::Docusaurus => "docusaurus",
            Target::GithubWiki => "github-wiki",
        }
    }

    pub(super) fn flat(self) -> bool {
        self == Target::GithubWiki
    }

    /// Whether headings keep their `{#anchor}` (MkDocs with attr_list, Docusaurus).
    pub(super) fn keeps_anchors(self) -> bool {
        matches!(self, Target::Mkdocs | Target::Docusaurus)
    }
}

/// What to export and how.
#[derive(Debug, Clone)]
pub struct Options {
    pub target: Target,
    /// Include documents in the git-ignored local directory.
    pub local: bool,
    /// Start each page with a note naming its source file.
    pub banner: bool,
}

/// A reference written as plain text because it leads nowhere in the tree.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Unresolved {
    pub path: String,
    pub raw: String,
    pub reason: String,
}

/// The built export: its files and the references written as text.
#[derive(Debug, Clone, Default)]
pub struct Tree {
    /// Relative path and content, in path order.
    pub files: BTreeMap<String, String>,
    pub unresolved: Vec<Unresolved>,
    pub docs: usize,
}
