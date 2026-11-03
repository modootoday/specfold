//! The whole graph in Graphviz DOT.

use super::run::governs;
use crate::session::Session;

pub(super) fn esc_dot(t: &str) -> String {
    t.replace('\\', "\\\\").replace('"', "\\\"")
}

/// C190: nodes get generated ids (`d0`, `p0`) with the text as the label, as in mermaid, so a
/// governs path spelled like a document id stays its own node.
pub(super) fn dot(s: &Session) -> Vec<String> {
    let c = &s.corpus;
    let mut l = vec!["digraph specfold {".to_string(), "  rankdir=LR;".to_string(), "  node [shape=box];".to_string()];
    for (i, d) in c.docs.iter().enumerate() {
        l.push(format!("  d{i} [label=\"{}\"];", esc_dot(&d.id.to_string())));
    }
    for e in &s.graph.edges {
        l.push(format!("  d{} -> d{} [label=\"{}\"];", e.from, e.to, esc_dot(&e.kind)));
    }
    let mut paths: Vec<String> = vec![];
    for (i, d) in c.docs.iter().enumerate() {
        for p in governs(s, d) {
            let n = match paths.iter().position(|x| *x == p) {
                Some(n) => n,
                None => {
                    l.push(format!("  p{} [label=\"{}\", shape=folder];", paths.len(), esc_dot(&p)));
                    paths.push(p);
                    paths.len() - 1
                }
            };
            l.push(format!("  d{i} -> p{n} [label=\"governs\", style=dashed];"));
        }
    }
    l.push("}".to_string());
    l
}
