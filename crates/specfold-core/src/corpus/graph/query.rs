//! Questions the graph answers: what governs a path, what a change would reach, where a
//! supersede chain ends.

pub use super::governs::{Governing, GoverningIndex};
use super::types::Graph;
use crate::corpus::Corpus;
use std::collections::{BTreeSet, VecDeque};

/// Documents whose `rel.governs` covers `path`, or, without `governs`, whose owner contains it.
/// Narrowest claims first. To ask about many paths, build one [`GoverningIndex`] and query it.
pub fn governing(c: &Corpus, path: &str) -> Vec<Governing> {
    GoverningIndex::new(c).governing(c, path)
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
