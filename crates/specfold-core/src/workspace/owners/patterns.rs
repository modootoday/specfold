//! Directory patterns: `services/*`-style kinds and path matchers for public and retired lists.

use globset::{Glob, GlobMatcher};
use std::path::Path;

/// A path or glob; a plain path also matches everything under it.
#[derive(Debug, Clone)]
pub enum PathMatch {
    Prefix(String),
    Glob(GlobMatcher),
}

impl PathMatch {
    pub fn is_match(&self, rel: &str) -> bool {
        match self {
            PathMatch::Prefix(p) => rel == p || rel.starts_with(&format!("{p}/")),
            PathMatch::Glob(g) => g.is_match(rel),
        }
    }
}

pub fn matcher(pattern: &str) -> Option<PathMatch> {
    let p = pattern.trim_end_matches('/');
    if p.contains(['*', '?', '[', '{']) {
        Glob::new(p).ok().map(|g| PathMatch::Glob(g.compile_matcher()))
    } else {
        Some(PathMatch::Prefix(p.to_string()))
    }
}

/// Expands a pattern of literal segments and `*` into existing directories, returning each with
/// the values its `*` segments took.
pub fn expand(root: &Path, pattern: &str) -> Vec<(String, Vec<String>)> {
    let mut acc: Vec<(String, Vec<String>)> = vec![(String::new(), vec![])];
    for seg in pattern.trim_matches('/').split('/') {
        let mut next = vec![];
        for (base, caps) in &acc {
            let dir = if base.is_empty() { root.to_path_buf() } else { root.join(base) };
            if seg == "*" {
                for name in subdirs(&dir) {
                    let mut c = caps.clone();
                    c.push(name.clone());
                    next.push((join(base, &name), c));
                }
            } else if dir.join(seg).is_dir() {
                next.push((join(base, seg), caps.clone()));
            }
        }
        acc = next;
    }
    acc
}

fn subdirs(dir: &Path) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    let mut names: Vec<String> = rd
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| !n.starts_with('.') && n != "node_modules" && n != "target")
        .collect();
    names.sort();
    names
}

fn join(base: &str, seg: &str) -> String {
    if base.is_empty() { seg.to_string() } else { format!("{base}/{seg}") }
}

/// Fills `{1}`, `{2}` … in an owner id template.
pub fn render_id(template: &str, caps: &[String]) -> String {
    caps.iter().enumerate().fold(template.to_string(), |s, (i, c)| s.replace(&format!("{{{}}}", i + 1), c))
}

pub fn has_manifest(dir: &Path, manifests: &[String]) -> bool {
    manifests.iter().any(|m| match m.strip_prefix("*.") {
        Some(ext) => std::fs::read_dir(dir)
            .map(|rd| rd.flatten().any(|e| e.path().extension().is_some_and(|x| x == ext)))
            .unwrap_or(false),
        None => dir.join(m).is_file(),
    })
}
