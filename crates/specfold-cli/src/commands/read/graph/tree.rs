//! The graph as indented trees from the documents nothing points at.

use super::run::governs;
use crate::session::Session;
use std::collections::BTreeSet;

pub(super) struct Glyphs {
    pub(super) tee: &'static str,
    pub(super) last: &'static str,
    pub(super) bar: &'static str,
    pub(super) arrow: &'static str,
}

/// Box drawing, or ASCII when asked or when the locale is not UTF-8.
pub(super) fn use_ascii(ascii: bool) -> bool {
    let locale = ["LC_ALL", "LC_CTYPE", "LANG"].iter().find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()));
    let utf8 = locale.is_none_or(|l| {
        let l = l.to_ascii_lowercase();
        l.contains("utf-8") || l.contains("utf8")
    });
    ascii || !utf8
}

pub(super) fn glyphs(ascii: bool) -> Glyphs {
    if use_ascii(ascii) {
        Glyphs { tee: "|-- ", last: "`-- ", bar: "|   ", arrow: "->" }
    } else {
        Glyphs { tee: "├─ ", last: "└─ ", bar: "│  ", arrow: "→" }
    }
}

/// The graph as trees from the documents nothing points at; a document shown before ends in (*).
pub(super) fn tree(s: &Session, max: Option<usize>, gl: Glyphs) -> Vec<String> {
    let c = &s.corpus;
    let mut order: Vec<usize> = (0..c.docs.len()).collect();
    order.sort_by_key(|&i| c.docs[i].id.to_string());
    let mut roots: Vec<usize> = order.iter().copied().filter(|&i| s.graph.edges_to(i).next().is_none()).collect();
    // Below a root at any depth; only documents on a cycle nothing points into start their own tree.
    let mut below: BTreeSet<usize> = roots.iter().copied().collect();
    let mut stack = roots.clone();
    while let Some(i) = stack.pop() {
        stack.extend(s.graph.edges_from(i).map(|e| e.to).filter(|t| below.insert(*t)));
    }
    let mut seen = BTreeSet::new();
    let mut out = vec![];
    let walk = |root: usize, seen: &mut BTreeSet<usize>, out: &mut Vec<String>| {
        seen.insert(root);
        out.push(label(s, root));
        children(s, root, "", 1, max, &gl, seen, out);
    };
    for r in roots.drain(..) {
        walk(r, &mut seen, &mut out);
    }
    for i in order {
        if !seen.contains(&i) && !below.contains(&i) {
            below.insert(i);
            let mut stack = vec![i];
            while let Some(j) = stack.pop() {
                stack.extend(s.graph.edges_from(j).map(|e| e.to).filter(|t| below.insert(*t)));
            }
            walk(i, &mut seen, &mut out);
        }
    }
    out
}

fn label(s: &Session, i: usize) -> String {
    let d = &s.corpus.docs[i];
    format!("{}  [{}]", d.id, d.status().unwrap_or_default())
}

#[allow(clippy::too_many_arguments)]
fn children(
    s: &Session,
    i: usize,
    prefix: &str,
    depth: usize,
    max: Option<usize>,
    gl: &Glyphs,
    seen: &mut BTreeSet<usize>,
    out: &mut Vec<String>,
) {
    if max.is_some_and(|m| depth > m) {
        return;
    }
    let mut kids: Vec<(String, Option<usize>, String)> =
        s.graph.edges_from(i).map(|e| (e.kind.clone(), Some(e.to), label(s, e.to))).collect();
    kids.extend(governs(s, &s.corpus.docs[i]).into_iter().map(|p| ("governs".to_string(), None, p)));
    let n = kids.len();
    for (k, (kind, to, text)) in kids.into_iter().enumerate() {
        let (lead, more) = if k + 1 == n { (gl.last, "    ") } else { (gl.tee, gl.bar) };
        let repeat = to.is_some_and(|t| !seen.insert(t));
        out.push(format!("{prefix}{lead}{kind} {} {text}{}", gl.arrow, if repeat { " (*)" } else { "" }));
        if let (Some(t), false) = (to, repeat) {
            children(s, t, &format!("{prefix}{more}"), depth + 1, max, gl, seen, out);
        }
    }
}
