//! The graph queries: impact (who reaches a document), governs (who covers a path) and consumers
//! (who points at a document here and in remotes).

use super::tree::glyphs;
use crate::cli::args::read::GraphArgs;
use crate::output::Output;
use crate::session::{self, Session};
use anyhow::{Result, bail};
use serde_json::{Value, json};
use specfold_core::corpus::graph::query;
use std::collections::{BTreeSet, VecDeque};

/// `impact`: the documents that reach the target, and the agent assets that read them.
pub(super) fn impact(s: &Session, a: &GraphArgs, t: &str, mut o: Output) -> Result<Output> {
    let c = &s.corpus;
    let id = |i: usize| c.docs[i].id.to_string();
    let idx = match session::resolve(s, t) {
        Ok(i) => i,
        // A code path is not a document; governs answers which documents cover it.
        Err(e) if !t.contains(':') && (t.contains('/') || c.cfg.spec_home.join(t).exists()) => {
            bail!("{e}; next: spec graph governs {t}")
        }
        Err(e) => return Err(e),
    };
    let arrow = glyphs(a.ascii).arrow;
    let reached = reach(s, idx, a.depth);
    // Agent assets that name the document or anything reaching it read it too, so a
    // change can break them; they are not graph nodes, so they come after the documents.
    let mut touched: Vec<usize> = vec![idx];
    touched.extend(reached.iter().map(|r| r.idx));
    let assets: Vec<Value> = specfold_core::assets::load(&c.cfg)
        .iter()
        .filter_map(|x| {
            let named = x.docs(&s.graph);
            let hit: Vec<String> = touched.iter().filter(|i| named.contains(i)).map(|i| id(*i)).collect();
            (!hit.is_empty()).then(|| json!({"kind": x.kind.as_str(), "name": x.name(), "path": x.path, "names": hit}))
        })
        .collect();
    for x in &assets {
        let names: Vec<&str> = x["names"]
            .as_array()
            .expect("graph nodes always carry a names array")
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        o = o.foot(format!(
            "asset {} {}  names {}",
            x["kind"].as_str().unwrap_or(""),
            x["path"].as_str().unwrap_or(""),
            names.join(", ")
        ));
    }
    o = o.field("assets", Value::Array(assets));
    for r in reached {
        o = o.item(
            json!({"id": id(r.idx), "depth": r.depth, "kind": r.kind, "via": id(r.via), "status": c.docs[r.idx].status()}),
            format!("{}{}  {} {arrow} {}  ({})", "  ".repeat(r.depth), id(r.idx), r.kind, id(r.via), r.depth),
        );
    }
    Ok(o.head(format!("documents that reach {} (relation, distance):", id(idx)))
        .next(format!("spec show {} --rel", specfold_core::suggest::shell::word(id(idx)))))
}

/// `governs`: the documents that govern a path.
pub(super) fn governs_query(s: &Session, t: &str, mut o: Output) -> Result<Output> {
    let c = &s.corpus;
    let id = |i: usize| c.docs[i].id.to_string();
    let rel = s.loc.code_path(&s.corpus.cfg, t)?;
    for gv in query::governing(c, &rel) {
        o = o.item(
            json!({"id": id(gv.idx), "pattern": gv.pattern, "status": c.docs[gv.idx].status()}),
            format!("{:<36} {}", id(gv.idx), gv.pattern),
        );
    }
    Ok(o.next(format!("spec context {}", specfold_core::suggest::shell::word(&rel))))
}

/// `consumers`: the documents here and in remotes that point at the target.
pub(super) fn consumers(s: &Session, t: &str, mut o: Output) -> Result<Output> {
    let c = &s.corpus;
    let id = |i: usize| c.docs[i].id.to_string();
    use specfold_core::workspace::remotes::{self, Remote};
    let (shown, file) = match remotes::resolve(&c.cfg, t) {
        Remote::Local => {
            let idx = session::resolve(s, t)?;
            (id(idx), c.cfg.spec_home.join(&c.docs[idx].path))
        }
        Remote::Found { name, id, file, .. } => (format!("{name}:{id}"), file),
        Remote::Absent { name, path } => {
            bail!("the remote {name} is not on disk ({}); next: spec doctor", path.display())
        }
        Remote::Missing { name, id } | Remote::MissingAnchor { name, id, .. } => {
            bail!("the remote {name} has no document {id}; next: spec doctor")
        }
        Remote::Ambiguous { name, ids } => bail!("{t} is ambiguous in the remote {name}: {}", ids.join(", ")),
    };
    let found = remotes::consumers(&c.cfg, c, &file);
    let names: Vec<&String> = c.cfg.raw.remotes.keys().collect();
    let scope = if names.is_empty() {
        "this repository only; no [remotes] in spec.toml".to_string()
    } else {
        format!(
            "this repository and the remotes on disk: {}",
            names.iter().map(|n| n.as_str()).collect::<Vec<_>>().join(", ")
        )
    };
    if !found.is_empty() {
        o = o.head(format!("{:<12} {:<36} {}", "repo", "id", "via"));
    }
    let count = found.len();
    for u in found {
        o = o.item(
            json!({"repo": u.repo, "id": u.id, "path": u.path, "via": u.via}),
            format!("{:<12} {:<36} {}", u.repo, u.id, u.via),
        );
    }
    let noun = if count == 1 { "consumer" } else { "consumers" };
    o = o.foot(format!("{count} {noun} of {shown}; searched {scope}"));
    if count > 0 {
        o = o.foot("repo '.' is this repository");
    }
    Ok(o.field("target", shown.clone())
        .field("searched", scope)
        .next(format!("spec show {}", specfold_core::suggest::shell::word(&shown))))
}

struct Reach {
    idx: usize,
    depth: usize,
    kind: String,
    via: usize,
}

/// Breadth-first over incoming relations: who reaches `idx`, by which relation, how far away.
fn reach(s: &Session, idx: usize, max: Option<usize>) -> Vec<Reach> {
    let mut seen = BTreeSet::from([idx]);
    let (mut out, mut q) = (vec![], VecDeque::from([(idx, 0usize)]));
    while let Some((cur, depth)) = q.pop_front() {
        if max.is_some_and(|m| depth >= m) {
            continue;
        }
        for e in s.graph.edges_to(cur) {
            if seen.insert(e.from) {
                out.push(Reach { idx: e.from, depth: depth + 1, kind: e.kind.clone(), via: cur });
                q.push_back((e.from, depth + 1));
            }
        }
    }
    out
}
