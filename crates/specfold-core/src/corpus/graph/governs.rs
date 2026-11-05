//! What governs a path: each document's `rel.governs` compiled once and filed under the literal
//! folder a match must lie in, so asking about many paths stays linear in them.

use crate::corpus::Corpus;
use crate::workspace::owners::{PathMatch, matcher};
use std::collections::BTreeMap;

/// A document that governs a path, and how specific its claim is (longer = narrower).
#[derive(Debug, Clone)]
pub struct Governing {
    pub idx: usize,
    pub pattern: String,
    pub specificity: usize,
}

/// One `governs` entry of a document, compiled.
struct Claim {
    pattern: String,
    matcher: Option<PathMatch>,
    specificity: usize,
}

/// How a document claims paths: its `governs` entries, or, without them, its owner directory.
enum Claims {
    Globs(Vec<Claim>),
    Owner(String),
}

/// Documents' claims compiled once; [`GoverningIndex::governing`] answers [`super::query::governing`]
/// for one path at a time, with the same result and order.
pub struct GoverningIndex {
    docs: Vec<(usize, Claims)>,
    /// Positions in `docs` of documents with a claim that only this path and paths below it match.
    by_prefix: BTreeMap<String, Vec<usize>>,
    /// Positions in `docs` of documents with a claim that may match any path.
    anywhere: Vec<usize>,
}

impl GoverningIndex {
    /// Every document of the corpus.
    pub fn new(c: &Corpus) -> Self {
        Self::filtered(c, |_| true)
    }

    /// Only the documents `keep` accepts (by index); the others are never returned.
    pub fn filtered(c: &Corpus, keep: impl Fn(usize) -> bool) -> Self {
        let mut ix = GoverningIndex { docs: vec![], by_prefix: BTreeMap::new(), anywhere: vec![] };
        for (i, d) in c.docs.iter().enumerate().filter(|(i, _)| keep(*i)) {
            let globs = d.rel().remove("governs").unwrap_or_default();
            let claims = if globs.is_empty() {
                match c.owners.by_id(&d.id.owner) {
                    Some(o) if !o.path.is_empty() => Claims::Owner(o.path.clone()),
                    _ => continue,
                }
            } else {
                Claims::Globs(globs.into_iter().map(compile).collect())
            };
            let pos = ix.docs.len();
            let mut keys: Vec<Option<String>> = match &claims {
                Claims::Globs(list) => list.iter().map(|g| prefix_key(&g.pattern)).collect(),
                Claims::Owner(o) => vec![Some(o.clone())],
            };
            keys.sort();
            keys.dedup();
            for key in keys {
                match key {
                    Some(k) => ix.by_prefix.entry(k).or_default().push(pos),
                    None => ix.anywhere.push(pos),
                }
            }
            ix.docs.push((i, claims));
        }
        ix
    }

    /// Documents whose claims cover `path`, narrowest first.
    pub fn governing(&self, c: &Corpus, path: &str) -> Vec<Governing> {
        let path = path.trim_start_matches("./").trim_end_matches('/');
        // C207: for a folder, a document governing a file or glob inside it governs part of it.
        let inside =
            (!path.is_empty() && c.cfg.spec_home.join(path).is_dir()).then(|| files_under(&c.cfg.spec_home, path));
        let candidates = self.candidates(path, inside.is_some());
        let mut out = vec![];
        for (i, claims) in candidates.into_iter().map(|p| &self.docs[p]) {
            match claims {
                Claims::Owner(owner) => {
                    let under = inside.is_some() && crate::paths::is_below(owner, path);
                    if crate::paths::is_under(path, owner) || under {
                        out.push(Governing { idx: *i, pattern: format!("{owner}/**"), specificity: owner.len() });
                    }
                }
                Claims::Globs(list) => {
                    let hit = list.iter().find(|g| {
                        g.matcher.as_ref().is_some_and(|m| m.is_match(path))
                            || inside.as_ref().is_some_and(|files| below_dir(g, path, files))
                    });
                    if let Some(g) = hit {
                        out.push(Governing { idx: *i, pattern: g.pattern.clone(), specificity: g.specificity });
                    }
                }
            }
        }
        out.sort_by(|a, b| b.specificity.cmp(&a.specificity).then(a.idx.cmp(&b.idx)));
        out
    }
}

impl GoverningIndex {
    /// Positions of the documents that may claim `path`: those claiming anywhere, those keyed by
    /// the path or a folder above it, and for a folder those keyed below it (C207).
    fn candidates(&self, path: &str, folder: bool) -> Vec<usize> {
        if path.is_empty() {
            return (0..self.docs.len()).collect();
        }
        let mut out = self.anywhere.clone();
        let above = path.match_indices('/').map(|(i, _)| &path[..i]).filter(|p| !p.is_empty());
        for key in above.chain(std::iter::once(path)) {
            out.extend(self.by_prefix.get(key).into_iter().flatten());
        }
        if folder {
            let below = format!("{path}/");
            let keyed = self.by_prefix.range(below.clone()..).take_while(|(k, _)| k.starts_with(&below));
            out.extend(keyed.flat_map(|(_, v)| v));
        }
        out.sort_unstable();
        out.dedup();
        out
    }
}

fn compile(pattern: String) -> Claim {
    let specificity = literal(&pattern).len();
    Claim { matcher: matcher(&pattern), pattern, specificity }
}

/// The text of a glob before its first wildcard.
fn literal(g: &str) -> &str {
    g.split(['*', '?', '[', '{']).next().unwrap_or("")
}

/// The literal folder (or, without wildcards, the path) that every path a `governs` entry
/// claims is or lies below, or None when the entry may claim paths anywhere (a bare file-name
/// glob, a wildcard in the first segment, an escape, a pattern that does not compile).
pub(crate) fn prefix_key(pattern: &str) -> Option<String> {
    let p = pattern.trim_end_matches('/');
    let key = if p.contains(['*', '?', '[', '{']) {
        let lit = literal(p);
        if !p.contains('/') || lit.contains('\\') || matcher(pattern).is_none() {
            return None;
        }
        lit.rsplit_once('/')?.0
    } else {
        p
    };
    (!key.is_empty()).then(|| key.to_string())
}

/// Whether claim `g` covers something below folder `dir`: its literal part lies inside it, or it
/// matches one of the files there.
fn below_dir(g: &Claim, dir: &str, files: &[String]) -> bool {
    if crate::paths::is_below(literal(&g.pattern), dir) {
        return true;
    }
    g.matcher.as_ref().is_some_and(|m| files.iter().any(|f| m.is_match(f)))
}

/// Up to a few thousand file paths below `dir`, relative to the root, skipping dot folders.
fn files_under(root: &std::path::Path, dir: &str) -> Vec<String> {
    const MAX: usize = 5000;
    let mut out = vec![];
    let mut stack = vec![dir.to_string()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(root.join(&d)) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let rel = format!("{d}/{name}");
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                stack.push(rel);
            } else if out.len() < MAX {
                out.push(rel);
            }
        }
        if out.len() >= MAX {
            break;
        }
    }
    out
}
