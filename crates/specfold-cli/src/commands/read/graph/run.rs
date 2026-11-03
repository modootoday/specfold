//! `spec graph`: picks the query, and draws the whole graph as a tree, boxes, Mermaid or DOT.

use super::dot::{dot, esc_dot};
use super::mermaid::{esc_mermaid, mermaid};
use super::queries::{consumers, governs_query, impact};
use super::tree::{glyphs, tree, use_ascii};
use super::{boxes, chain};
use crate::cli::args::{global::Global, read::GraphArgs};
use crate::output::Output;
use crate::session::{self, Session};
use anyhow::{Result, bail};
use serde_json::json;

pub fn run(g: &Global, a: &GraphArgs) -> Result<Output> {
    let s = session::open(g)?;
    let c = &s.corpus;
    let o = Output::new("graph");
    if a.query.is_some() && a.format != "tree" {
        bail!("--format {} draws the whole graph: spec graph --format {}", a.format, a.format);
    }
    match (a.query.as_deref(), a.target.as_deref()) {
        (None, _) => whole(&s, a, g.jsonl, o),
        (Some("impact"), Some(t)) => impact(&s, a, t, o),
        (Some("chain"), Some(t)) => chain::chain(&s, t, o),
        (Some("governs"), Some(t)) => governs_query(&s, t, o),
        (Some("consumers"), Some(t)) => consumers(&s, t, o),
        (Some(q), _) if !["impact", "chain", "governs", "consumers"].contains(&q) => {
            let whole = match q {
                "tree" => "spec graph".to_string(),
                "ascii" => "spec graph --ascii".to_string(),
                "mermaid" | "dot" | "box" => format!("spec graph --format {q}"),
                "json" => "spec graph --json".to_string(),
                _ => "spec graph impact <id>".to_string(),
            };
            bail!(
                "unknown query '{q}' (impact, chain, governs, consumers; the whole graph takes no query); next: {whole}"
            )
        }
        (Some(q), None) => {
            let example = if q == "governs" {
                c.docs.iter().flat_map(|d| governs(&s, d)).next()
            } else {
                session::example_doc(&s, (q == "chain").then_some("adr"))
            };
            match example {
                Some(t) => bail!("'{q}' needs a target: spec graph {q} <id or path>; next: spec graph {q} {t}"),
                None => bail!(
                    "'{q}' needs a target, and there are no documents yet; next: spec new adr --title 'First decision' --write"
                ),
            }
        }
        (Some(_), Some(_)) => unreachable!("every query is matched above"),
    }
}

/// The whole graph: a tree, box drawing, Mermaid or DOT.
fn whole(s: &Session, a: &GraphArgs, jsonl: bool, mut o: Output) -> Result<Output> {
    let c = &s.corpus;
    let id = |i: usize| c.docs[i].id.to_string();
    o = o.field("nodes", json!(c.docs.iter().map(|d| json!({"id": d.id.to_string(), "type": d.id.ty, "owner": d.id.owner, "status": d.status(), "path": d.path, "governs": governs(s, d)})).collect::<Vec<_>>()));
    o = o.field(
        "edges",
        json!(
            s.graph
                .edges
                .iter()
                .map(|e| json!({"from": id(e.from), "to": id(e.to), "kind": e.kind, "anchor": e.anchor}))
                .collect::<Vec<_>>()
        ),
    );
    let gov: Vec<_> = c
        .docs
        .iter()
        .flat_map(|d| governs(s, d).into_iter().map(move |p| json!({"from": d.id.to_string(), "path": p})))
        .collect();
    if jsonl {
        // C212: a stream gets one row per node, edge and governed path, tagged by `row`.
        o = stream_rows(&o, &gov);
    } else {
        o = o.unpaged();
    }
    o = o.field("governs", json!(gov));
    let named = if a.assets { asset_nodes(s) } else { vec![] };
    if a.assets {
        o = o.field(
            "assets",
            json!(named.iter().map(|(k, n, p, d)| json!({"kind": k, "name": n, "path": p, "names": d.iter().map(|i| id(*i)).collect::<Vec<_>>()})).collect::<Vec<_>>()),
        );
        if matches!(a.format.as_str(), "tree" | "box") {
            for (k, _, p, d) in &named {
                o = o.foot(format!("asset {k} {p}  names {}", d.iter().map(|i| id(*i)).collect::<Vec<_>>().join(", ")));
            }
        }
    }
    let lines = match a.format.as_str() {
        "mermaid" => {
            let mut l = mermaid(s);
            for (n, (k, _, p, d)) in named.iter().enumerate() {
                l.push(format!("  a{n}{{{{\"{} {}\"}}}}", k, esc_mermaid(p)));
                l.extend(d.iter().map(|i| format!("  a{n} -.->|names| d{i}")));
            }
            l
        }
        "dot" => {
            let mut l = dot(s);
            let close = l.pop();
            for (k, _, p, d) in &named {
                l.push(format!("  \"{}\" [shape=note, label=\"{} {}\"];", esc_dot(p), k, esc_dot(p)));
                for i in d {
                    l.push(format!(
                        "  \"{}\" -> \"{}\" [label=\"names\", style=dotted];",
                        esc_dot(p),
                        esc_dot(&id(*i))
                    ));
                }
            }
            l.extend(close);
            l
        }
        "box" => {
            let labels: Vec<String> =
                c.docs.iter().map(|d| format!("{}\n{}", d.id, d.status().unwrap_or_default())).collect();
            let edges: Vec<_> = s.graph.edges.iter().map(|e| (e.from, e.to, e.kind.clone())).collect();
            let columns = std::env::var("COLUMNS").ok().and_then(|v| v.parse().ok()).unwrap_or(100);
            let why = match boxes::draw(&labels, &edges, a.depth, columns, use_ascii(a.ascii)) {
                Ok(d) => {
                    o = o.field("diagram", d.lines.join("\n"));
                    return Ok(d
                        .lines
                        .into_iter()
                        .fold(o, |o, x| o.head(x))
                        .next("spec graph  # the tree, with governs paths"));
                }
                Err(boxes::Skip::TooMany(k)) => {
                    format!("{k} documents is more than a box drawing holds ({}); add --depth N", boxes::MAX_NODES)
                }
                Err(boxes::Skip::TooWide { need, have }) => {
                    format!("boxes need {need} columns, COLUMNS is {have}; add --depth N or widen the terminal")
                }
            };
            let mut l = vec![format!("{why}; showing the tree")];
            l.extend(tree(s, a.depth, glyphs(a.ascii)));
            return Ok(l.into_iter().fold(o, |o, x| o.head(x)).next("spec graph --format mermaid"));
        }
        _ => {
            let mut l = vec![format!("{} documents, {} edges", c.docs.len(), s.graph.edges.len())];
            l.extend(tree(s, a.depth, glyphs(a.ascii)));
            return Ok(l.into_iter().fold(o, |o, x| o.head(x)).next(match c.docs.first() {
                Some(d) => format!("spec graph impact {}", specfold_core::shell::word(&d.id)),
                None => "spec graph".into(),
            }));
        }
    };
    o = o.field("diagram", lines.join("\n"));
    Ok(lines.into_iter().fold(o, |o, x| o.head(x)))
}

pub(super) fn governs(_s: &Session, d: &specfold_core::model::Doc) -> Vec<String> {
    d.rel().remove("governs").unwrap_or_default()
}

/// Agent assets that name at least one document: (kind, name, path, documents named).
fn asset_nodes(s: &Session) -> Vec<(String, String, String, Vec<usize>)> {
    specfold_core::assets::load(&s.corpus.cfg)
        .iter()
        .filter_map(|x| {
            let mut d: Vec<usize> = x.docs(&s.graph).into_iter().collect();
            d.sort();
            (!d.is_empty()).then(|| (x.kind.as_str().to_string(), x.name().to_string(), x.path.to_string(), d))
        })
        .collect()
}

/// C212: the whole graph as stream rows for --jsonl.
fn stream_rows(o: &crate::output::Output, gov: &[serde_json::Value]) -> crate::output::Output {
    let mut out = o.clone();
    for (row, key) in [("node", "nodes"), ("edge", "edges")] {
        for v in o.summary.get(key).and_then(serde_json::Value::as_array).cloned().unwrap_or_default() {
            out = out.item(tagged(row, v), String::new());
        }
    }
    for v in gov {
        out = out.item(tagged("governs", v.clone()), String::new());
    }
    out
}

fn tagged(row: &str, mut v: serde_json::Value) -> serde_json::Value {
    if let serde_json::Value::Object(m) = &mut v {
        m.insert("row".into(), serde_json::json!(row));
    }
    v
}
