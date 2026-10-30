//! One asset file as read: its kind, location, frontmatter and text.

use super::join;
use crate::config::types::AssetKind;
use crate::graph::{Graph, Resolution};
use crate::markdown::frontmatter;
use serde_json::{Map, Value};
use std::collections::BTreeSet;

#[derive(Debug, Clone)]
pub struct Asset {
    pub kind: AssetKind,
    /// Repository-relative path of the file read (SKILL.md for a skill).
    pub path: String,
    /// The name the location gives: a skill's directory, otherwise the file stem.
    pub stem: String,
    /// Frontmatter; None when the file has none.
    pub fields: Option<Map<String, Value>>,
    /// Why the frontmatter could not be read.
    pub invalid: Option<String>,
    /// 1-based line where the body starts.
    pub body_line: usize,
    pub text: String,
}

impl Asset {
    /// A frontmatter string field.
    pub fn field(&self, key: &str) -> Option<&str> {
        self.fields.as_ref()?.get(key)?.as_str()
    }

    /// The declared name, or the one the location gives.
    pub fn name(&self) -> &str {
        self.field("name").unwrap_or(&self.stem)
    }

    pub fn body(&self) -> &str {
        frontmatter::split(&self.text).body
    }

    /// The documents this asset names, by index: a token that is a document id or path, written
    /// from the repository root or relative to the asset. Aliases are left out so a common word
    /// is never a link. Only read: nothing is added to the graph.
    pub fn docs(&self, g: &Graph) -> Vec<usize> {
        let dir = self.path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
        let mut out = BTreeSet::new();
        let ok = |c: char| c.is_ascii_alphanumeric() || ":/._#-".contains(c);
        for raw in self.text.split(|c: char| !ok(c)) {
            let t = raw.trim_end_matches(['.', ':', '#']);
            if !(t.contains(':') || t.contains('/') || t.ends_with(".md")) || t.contains("://") {
                continue;
            }
            for cand in [t.to_string(), join(dir, t)] {
                if let Resolution::Doc { idx, via_alias: false, .. } = g.resolve(&cand) {
                    out.insert(idx);
                    break;
                }
            }
        }
        out.into_iter().collect()
    }
}
