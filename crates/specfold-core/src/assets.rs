//! The asset corpus: agent skills, rules, subagents and instruction files declared under
//! `[[assets]]` in spec.toml. Assets have no ids and are only read; checks report on them but
//! nothing in specfold writes them.

use crate::config::Config;
use crate::config::types::{AssetKind, AssetRoot};
use crate::markdown::frontmatter::{self, Parsed};
use crate::workspace::owners::{PathMatch, matcher};
use ignore::WalkBuilder;
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::path::Path;

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
}

/// Whether a file found under a root has the shape of its kind.
fn shaped(kind: AssetKind, rel: &str) -> bool {
    let file = rel.rsplit('/').next().unwrap_or(rel);
    match kind {
        AssetKind::Skill => file == "SKILL.md",
        AssetKind::Rule => file.ends_with(".md") || file.ends_with(".mdc"),
        AssetKind::Agent | AssetKind::Instructions => file.ends_with(".md"),
    }
}

fn stem(kind: AssetKind, rel: &str) -> String {
    let mut parts = rel.rsplit('/');
    let file = parts.next().unwrap_or(rel);
    match kind {
        AssetKind::Skill => parts.next().unwrap_or("").to_string(),
        _ => file.rsplit_once('.').map(|(s, _)| s).unwrap_or(file).to_string(),
    }
}

/// The directory to walk: the path itself, or the part of a glob before its first wildcard.
fn base(path: &str) -> String {
    let lit: Vec<&str> = path.split('/').take_while(|c| !c.contains(['*', '?', '[', '{'])).collect();
    lit.join("/")
}

fn files(root: &Path, def: &AssetRoot) -> Vec<String> {
    let Some(want) = matcher(&def.path) else { return vec![] };
    let exclude: Vec<PathMatch> = def.exclude.iter().filter_map(|e| matcher(e)).collect();
    let start = root.join(base(def.path.trim_end_matches('/')));
    if !start.exists() {
        return vec![];
    }
    let mut out = vec![];
    for e in WalkBuilder::new(&start).hidden(false).build().flatten() {
        if !e.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let Ok(rel) = e.path().strip_prefix(root) else { continue };
        let rel = rel.to_string_lossy().replace('\\', "/");
        if want.is_match(&rel) && !exclude.iter().any(|m| m.is_match(&rel)) && shaped(def.kind, &rel) {
            out.push(rel);
        }
    }
    out
}

fn read(root: &Path, kind: AssetKind, rel: String) -> Option<Asset> {
    let text = std::fs::read_to_string(root.join(&rel)).ok()?;
    let split = frontmatter::split(&text);
    let body_line = split.body_line;
    let (fields, invalid) = match frontmatter::parse(split.raw) {
        Parsed::None => (None, None),
        Parsed::Map(m) => (Some(m), None),
        Parsed::Invalid(why) => (None, Some(why)),
    };
    Some(Asset { kind, stem: stem(kind, &rel), path: rel, fields, invalid, body_line, text })
}

/// Every declared asset, sorted by path; a file claimed by two roots is read once, by the first.
pub fn load(cfg: &Config) -> Vec<Asset> {
    let mut seen = BTreeSet::new();
    let mut out = vec![];
    for def in &cfg.raw.assets {
        for rel in files(&cfg.root, def) {
            if seen.insert(rel.clone())
                && let Some(a) = read(&cfg.root, def.kind, rel)
            {
                out.push(a);
            }
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stems_and_bases() {
        assert_eq!(stem(AssetKind::Skill, ".agents/skills/pdf-tools/SKILL.md"), "pdf-tools");
        assert_eq!(stem(AssetKind::Rule, ".cursor/rules/style.mdc"), "style");
        assert_eq!(base(".claude/agents/*.md"), ".claude/agents");
        assert_eq!(base("AGENTS.md"), "AGENTS.md");
        assert!(!shaped(AssetKind::Skill, ".agents/skills/x/notes.md"));
    }
}
