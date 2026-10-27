//! A loaded document: where it lives, what its frontmatter says, and what its body points at.

use super::id::DocId;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    /// Committed to the repository.
    Committed,
    /// In the git-ignored local directory.
    Local,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileEntry {
    /// Relative to the document's bundle directory.
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
    /// `repo` or `store`.
    pub store: String,
}

/// Something found in a document's files, with the file and line it came from.
#[derive(Debug, Clone, Serialize)]
pub struct Located<T> {
    pub path: String,
    pub line: usize,
    pub item: T,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Frontmatter {
    /// Present when the file has a frontmatter block.
    pub map: Option<Map<String, Value>>,
    /// A frontmatter block that does not parse.
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Doc {
    pub id: DocId,
    /// The main Markdown file, relative to the repository root.
    pub path: String,
    /// The bundle directory of a directory-form document.
    pub dir: Option<String>,
    pub tier: Tier,
    /// The slug part of the file name.
    pub slug: String,
    pub frontmatter: Frontmatter,
    pub title: Option<String>,
    /// Chapter files inside the bundle.
    pub chapters: Vec<String>,
    /// Non-Markdown files inside the bundle, relative to the repository root.
    pub bundle_files: Vec<String>,
    pub links: Vec<Located<String>>,
    pub anchors: Vec<Located<String>>,
    pub path_cites: Vec<Located<String>>,
    pub body_lines: usize,
    /// Hash of the main file's body, for sealed documents.
    pub body_hash: String,
}

impl Doc {
    pub fn fm(&self) -> Option<&Map<String, Value>> {
        self.frontmatter.map.as_ref()
    }

    pub fn str_field(&self, key: &str) -> Option<String> {
        match self.fm()?.get(key)? {
            Value::String(s) => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            Value::Bool(b) => Some(b.to_string()),
            _ => None,
        }
    }

    pub fn list_field(&self, key: &str) -> Vec<String> {
        list_of(self.fm().and_then(|m| m.get(key)))
    }

    pub fn status(&self) -> Option<String> {
        self.str_field("status")
    }

    /// Typed relations: `rel.<kind>` lists.
    pub fn rel(&self) -> BTreeMap<String, Vec<String>> {
        let mut out = BTreeMap::new();
        if let Some(Value::Object(m)) = self.fm().and_then(|m| m.get("rel")) {
            for (k, v) in m {
                out.insert(k.clone(), list_of(Some(v)));
            }
        }
        out
    }

    pub fn files(&self) -> Vec<FileEntry> {
        let Some(Value::Array(items)) = self.fm().and_then(|m| m.get("files")) else { return vec![] };
        items
            .iter()
            .filter_map(|v| {
                let o = v.as_object()?;
                Some(FileEntry {
                    path: o.get("path")?.as_str()?.to_string(),
                    sha256: o.get("sha256").and_then(Value::as_str).unwrap_or("").to_string(),
                    bytes: o.get("bytes").and_then(Value::as_u64).unwrap_or(0),
                    store: o.get("store").and_then(Value::as_str).unwrap_or("repo").to_string(),
                })
            })
            .collect()
    }

    /// The directory attachments are relative to: the bundle, or the main file's directory.
    pub fn base_dir(&self) -> String {
        match &self.dir {
            Some(d) => d.clone(),
            None => self.path.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default(),
        }
    }
}

pub fn list_of(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Array(a)) => a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        Some(Value::String(s)) if !s.is_empty() => vec![s.clone()],
        _ => vec![],
    }
}
