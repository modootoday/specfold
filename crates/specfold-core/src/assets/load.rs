//! Reading the declared assets: the files each root matches, read once each, sorted by path.

use super::asset::Asset;
use crate::config::Config;
use crate::config::types::{AssetKind, AssetRoot};
use crate::markdown::frontmatter::{self, Parsed};
use crate::workspace::owners::{PathMatch, matcher};
use std::collections::BTreeSet;
use std::path::Path;

/// Whether a file found under a root has the shape of its kind.
pub(super) fn shaped(kind: AssetKind, rel: &str) -> bool {
    let file = rel.rsplit('/').next().unwrap_or(rel);
    match kind {
        AssetKind::Skill => file == "SKILL.md",
        AssetKind::Rule => file.ends_with(".md") || file.ends_with(".mdc"),
        AssetKind::Agent | AssetKind::Instructions => file.ends_with(".md"),
    }
}

pub(super) fn stem(kind: AssetKind, rel: &str) -> String {
    let mut parts = rel.rsplit('/');
    let file = parts.next().unwrap_or(rel);
    match kind {
        AssetKind::Skill => parts.next().unwrap_or("").to_string(),
        _ => file.rsplit_once('.').map(|(s, _)| s).unwrap_or(file).to_string(),
    }
}

/// The directory to walk: the path itself, or the part of a glob before its first wildcard.
pub(super) fn base(path: &str) -> String {
    let lit: Vec<&str> = path.split('/').take_while(|c| !c.contains(['*', '?', '[', '{'])).collect();
    lit.join("/")
}

fn files(cfg: &Config, def: &AssetRoot) -> Vec<String> {
    let root = &cfg.spec_home;
    let Some(want) = matcher(&def.path) else { return vec![] };
    let exclude: Vec<PathMatch> = def.exclude.iter().filter_map(|e| matcher(e)).collect();
    let start = root.join(base(def.path.trim_end_matches('/')));
    if !start.exists() {
        return vec![];
    }
    let mut out = vec![];
    for e in crate::workspace::walk(cfg, &start).build().flatten() {
        if !e.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let Some(rel) = crate::paths::rel_of(root, e.path()) else { continue };
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
        for rel in files(cfg, def) {
            if seen.insert(rel.clone())
                && let Some(a) = read(&cfg.spec_home, def.kind, rel)
            {
                out.push(a);
            }
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}
