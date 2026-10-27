//! `spec graph`: impact, chain, governs, or the whole graph.

use crate::cli::args::{global::Global, read::GraphArgs};
use crate::output::Output;
use crate::session;
use anyhow::{Result, bail};
use serde_json::json;
use specfold_core::graph::query;

pub fn run(g: &Global, a: &GraphArgs) -> Result<Output> {
    let s = session::open(g)?;
    let c = &s.corpus;
    let id = |i: usize| c.docs[i].id.to_string();
    let mut o = Output::new("graph");
    match (a.query.as_deref(), a.target.as_deref()) {
        (None, _) => {
            o = o.field("nodes", json!(c.docs.iter().map(|d| json!({"id": d.id.to_string(), "type": d.id.ty, "owner": d.id.owner, "status": d.status(), "path": d.path})).collect::<Vec<_>>()));
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
            o = o.unpaged().head(format!("{} documents, {} edges", c.docs.len(), s.graph.edges.len()));
            Ok(o.next("spec graph impact <id>"))
        }
        (Some("impact"), Some(t)) => {
            let idx = session::resolve(&s, t)?;
            for (i, depth) in query::impact(&s.graph, idx) {
                o = o.item(
                    json!({"id": id(i), "depth": depth, "status": c.docs[i].status()}),
                    format!("{}{}", "  ".repeat(depth), id(i)),
                );
            }
            Ok(o.head(format!("documents that reach {}:", id(idx))).next(format!("spec show {} --rel", id(idx))))
        }
        (Some("chain"), Some(t)) => {
            let idx = session::resolve(&s, t)?;
            let chain = query::chain(&s.graph, idx);
            for (n, i) in chain.iter().enumerate() {
                o = o.item(
                    json!({"id": id(*i), "status": c.docs[*i].status()}),
                    format!("{}{}", if n == 0 { "" } else { "superseded by " }, id(*i)),
                );
            }
            let last = id(*chain.last().unwrap_or(&idx));
            Ok(o.field("current", last.clone()).next(format!("spec show {last}")))
        }
        (Some("governs"), Some(t)) => {
            let rel = session::repo_path(&s, t);
            for gv in query::governing(c, &rel) {
                o = o.item(
                    json!({"id": id(gv.idx), "pattern": gv.pattern, "status": c.docs[gv.idx].status()}),
                    format!("{:<36} {}", id(gv.idx), gv.pattern),
                );
            }
            Ok(o.next(format!("spec context {rel}")))
        }
        (Some(q), None) => bail!("'{q}' needs a target: spec graph {q} <id or path>"),
        (Some(q), _) => bail!("unknown query '{q}' (impact, chain, governs)"),
    }
}
