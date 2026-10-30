//! What plugins receive: a compact, stable view of the corpus and graph.

use crate::corpus::Corpus;
use crate::graph::Graph;
use serde_json::{Value, json};

pub fn docs(c: &Corpus, only: Option<&[usize]>) -> Value {
    let pick: Vec<usize> = match only {
        Some(list) => list.to_vec(),
        None => (0..c.docs.len()).collect(),
    };
    Value::Array(pick.into_iter().map(|i| doc(c, i)).collect())
}

/// One document as every plugin sees it.
pub fn doc(c: &Corpus, i: usize) -> Value {
    let d = &c.docs[i];
    json!({
                    "id": d.id.to_string(),
                    "type": d.id.ty,
                    "owner": d.id.owner,
                    "path": d.path,
                    "chapters": d.chapters,
                    "title": d.title,
                    "status": d.status(),
                    "frontmatter": d.fm(),
        "anchors": d.anchors.iter().map(|a| &a.item).collect::<Vec<_>>(),
    })
}

pub fn edges(c: &Corpus, g: &Graph) -> Value {
    Value::Array(g.edges.iter().map(|e| json!({"from": c.docs[e.from].id.to_string(), "to": c.docs[e.to].id.to_string(), "kind": e.kind, "anchor": e.anchor})).collect())
}
