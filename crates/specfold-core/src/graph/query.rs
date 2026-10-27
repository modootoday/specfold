//! Questions the graph answers: what governs a path, what a change would reach, where a
//! supersede chain ends.

use super::types::Graph;
use crate::corpus::Corpus;
use crate::workspace::owners::matcher;
use std::collections::{BTreeSet, VecDeque};

/// A document that governs a path, and how specific its claim is (longer = narrower).
#[derive(Debug, Clone)]
pub struct Governing {
    pub idx: usize,
    pub pattern: String,
    pub specificity: usize,
}

/// Documents whose `rel.governs` covers `path`, or, without `governs`, whose owner contains it.
/// Narrowest claims first.
pub fn governing(c: &Corpus, path: &str) -> Vec<Governing> {
    let path = path.trim_start_matches("./").trim_end_matches('/');
    let mut out = vec![];
    for (i, d) in c.docs.iter().enumerate() {
        let globs = d.rel().remove("governs").unwrap_or_default();
        if globs.is_empty() {
            let Some(owner) = c.owners.by_id(&d.id.owner) else { continue };
            if !owner.path.is_empty() && (path == owner.path || path.starts_with(&format!("{}/", owner.path))) {
                out.push(Governing { idx: i, pattern: format!("{}/**", owner.path), specificity: owner.path.len() });
            }
            continue;
        }
        for g in globs {
            if matcher(&g).is_some_and(|m| m.is_match(path)) {
                let specificity = g.split(['*', '?', '[', '{']).next().unwrap_or("").len();
                out.push(Governing { idx: i, pattern: g, specificity });
                break;
            }
        }
    }
    out.sort_by(|a, b| b.specificity.cmp(&a.specificity).then(a.idx.cmp(&b.idx)));
    out
}

/// Every document that reaches `idx` through incoming edges, nearest first.
pub fn impact(g: &Graph, idx: usize) -> Vec<(usize, usize)> {
    let mut seen = BTreeSet::from([idx]);
    let mut out = vec![];
    let mut q = VecDeque::from([(idx, 0usize)]);
    while let Some((cur, depth)) = q.pop_front() {
        for e in g.edges_to(cur) {
            if seen.insert(e.from) {
                out.push((e.from, depth + 1));
                q.push_back((e.from, depth + 1));
            }
        }
    }
    out
}

/// A `supersedes` cycle through `idx`, as the documents in order, or None. In a cycle every
/// member is superseded, so nothing in it is current.
pub fn supersede_cycle(g: &Graph, idx: usize) -> Option<Vec<usize>> {
    let mut stack = vec![(idx, vec![idx])];
    let mut seen = BTreeSet::new();
    while let Some((cur, path)) = stack.pop() {
        for e in g.edges_from(cur).filter(|e| e.kind == "supersedes") {
            if e.to == idx {
                return Some(path);
            }
            if seen.insert(e.to) {
                let mut next = path.clone();
                next.push(e.to);
                stack.push((e.to, next));
            }
        }
    }
    None
}

/// Follows `supersedes` edges backwards from `idx` to the document that replaced it last.
pub fn chain(g: &Graph, idx: usize) -> Vec<usize> {
    let mut path = vec![idx];
    let mut cur = idx;
    loop {
        let next = g.edges_to(cur).find(|e| e.kind == "supersedes").map(|e| e.from);
        match next {
            Some(n) if !path.contains(&n) => {
                path.push(n);
                cur = n;
            }
            _ => return path,
        }
    }
}
