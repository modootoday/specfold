//! The whole graph as a Mermaid flowchart.

use super::run::governs;
use crate::session::Session;

pub(super) fn esc_mermaid(t: &str) -> String {
    t.replace('"', "#quot;")
}

pub(super) fn mermaid(s: &Session) -> Vec<String> {
    let c = &s.corpus;
    let mut l = vec!["graph LR".to_string()];
    for (i, d) in c.docs.iter().enumerate() {
        l.push(format!("  d{i}[\"{}\"]", esc_mermaid(&d.id.to_string())));
    }
    for e in &s.graph.edges {
        l.push(format!("  d{} -->|{}| d{}", e.from, esc_mermaid(&e.kind), e.to));
    }
    let mut paths: Vec<String> = vec![];
    for (i, d) in c.docs.iter().enumerate() {
        for p in governs(s, d) {
            let n = match paths.iter().position(|x| *x == p) {
                Some(n) => n,
                None => {
                    l.push(format!("  p{}[/\"{}\"/]", paths.len(), esc_mermaid(&p)));
                    paths.push(p);
                    paths.len() - 1
                }
            };
            l.push(format!("  d{i} -.->|governs| p{n}"));
        }
    }
    l
}
