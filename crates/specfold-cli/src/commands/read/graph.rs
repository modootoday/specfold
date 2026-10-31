//! `spec graph`: the whole graph as a tree, Mermaid or DOT; impact, chain and governs.

use super::graph_box;
use crate::cli::args::{global::Global, read::GraphArgs};
use crate::output::Output;
use crate::session::{self, Session};
use anyhow::{Result, bail};
use serde_json::{Value, json};
use specfold_core::graph::query;
use std::collections::{BTreeSet, VecDeque};

pub fn run(g: &Global, a: &GraphArgs) -> Result<Output> {
    let s = session::open(g)?;
    let c = &s.corpus;
    let o = Output::new("graph");
    if a.query.is_some() && a.format != "tree" {
        bail!("--format {} draws the whole graph: spec graph --format {}", a.format, a.format);
    }
    match (a.query.as_deref(), a.target.as_deref()) {
        (None, _) => whole(&s, a, o),
        (Some("impact"), Some(t)) => impact(&s, a, t, o),
        (Some("chain"), Some(t)) => chain(&s, t, o),
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
                    "'{q}' needs a target, and there are no documents yet; next: spec new adr --title 'First decision'"
                ),
            }
        }
        (Some(_), Some(_)) => unreachable!("every query is matched above"),
    }
}

/// The whole graph: a tree, box drawing, Mermaid or DOT.
fn whole(s: &Session, a: &GraphArgs, mut o: Output) -> Result<Output> {
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
    o = o.field("governs", json!(gov)).unpaged();
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
            let why = match graph_box::draw(&labels, &edges, a.depth, columns, use_ascii(a.ascii)) {
                Ok(d) => {
                    o = o.field("diagram", d.lines.join("\n"));
                    return Ok(d
                        .lines
                        .into_iter()
                        .fold(o, |o, x| o.head(x))
                        .next("spec graph  # the tree, with governs paths"));
                }
                Err(graph_box::Skip::TooMany(k)) => {
                    format!("{k} documents is more than a box drawing holds ({}); add --depth N", graph_box::MAX_NODES)
                }
                Err(graph_box::Skip::TooWide { need, have }) => {
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

/// `impact`: the documents that reach the target, and the agent assets that read them.
fn impact(s: &Session, a: &GraphArgs, t: &str, mut o: Output) -> Result<Output> {
    let c = &s.corpus;
    let id = |i: usize| c.docs[i].id.to_string();
    let idx = match session::resolve(s, t) {
        Ok(i) => i,
        // A code path is not a document; governs answers which documents cover it.
        Err(e) if !t.contains(':') && (t.contains('/') || c.cfg.root.join(t).exists()) => {
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
        .next(format!("spec show {} --rel", specfold_core::shell::word(id(idx)))))
}

/// `chain`: the supersede chain through the target.
fn chain(s: &Session, t: &str, mut o: Output) -> Result<Output> {
    let c = &s.corpus;
    let id = |i: usize| c.docs[i].id.to_string();
    let idx = session::resolve(s, t)?;
    let mut chain = older(s, idx);
    chain.reverse();
    chain.extend(query::chain(&s.graph, idx));
    for (n, i) in chain.iter().enumerate() {
        let this = if *i == idx { "  (this)" } else { "" };
        o = o.item(
            json!({"id": id(*i), "status": c.docs[*i].status(), "this": *i == idx}),
            format!("{}{}{this}", if n == 0 { "" } else { "superseded by " }, id(*i)),
        );
    }
    let last = id(*chain.last().unwrap_or(&idx));
    Ok(o.field("current", last.clone()).next(format!("spec show {}", specfold_core::shell::word(&last))))
}

/// `governs`: the documents that govern a path.
fn governs_query(s: &Session, t: &str, mut o: Output) -> Result<Output> {
    let c = &s.corpus;
    let id = |i: usize| c.docs[i].id.to_string();
    let rel = session::repo_path(s, t);
    for gv in query::governing(c, &rel) {
        o = o.item(
            json!({"id": id(gv.idx), "pattern": gv.pattern, "status": c.docs[gv.idx].status()}),
            format!("{:<36} {}", id(gv.idx), gv.pattern),
        );
    }
    Ok(o.next(format!("spec context {}", specfold_core::shell::word(&rel))))
}

/// `consumers`: the documents here and in remotes that point at the target.
fn consumers(s: &Session, t: &str, mut o: Output) -> Result<Output> {
    let c = &s.corpus;
    let id = |i: usize| c.docs[i].id.to_string();
    use specfold_core::remotes::{self, Remote};
    let (shown, file) = match remotes::resolve(&c.cfg, t) {
        Remote::Local => {
            let idx = session::resolve(s, t)?;
            (id(idx), c.cfg.root.join(&c.docs[idx].path))
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
        .next(format!("spec show {}", specfold_core::shell::word(&shown))))
}

fn governs(_s: &Session, d: &specfold_core::model::Doc) -> Vec<String> {
    d.rel().remove("governs").unwrap_or_default()
}

/// The documents `idx` supersedes, nearest first.
fn older(s: &Session, idx: usize) -> Vec<usize> {
    let (mut out, mut cur) = (vec![], idx);
    while let Some(n) = s.graph.edges_from(cur).find(|e| e.kind == "supersedes").map(|e| e.to) {
        if n == idx || out.contains(&n) {
            break;
        }
        out.push(n);
        cur = n;
    }
    out
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

struct Glyphs {
    tee: &'static str,
    last: &'static str,
    bar: &'static str,
    arrow: &'static str,
}

/// Box drawing, or ASCII when asked or when the locale is not UTF-8.
fn use_ascii(ascii: bool) -> bool {
    let locale = ["LC_ALL", "LC_CTYPE", "LANG"].iter().find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()));
    let utf8 = locale.is_none_or(|l| {
        let l = l.to_ascii_lowercase();
        l.contains("utf-8") || l.contains("utf8")
    });
    ascii || !utf8
}

fn glyphs(ascii: bool) -> Glyphs {
    if use_ascii(ascii) {
        Glyphs { tee: "|-- ", last: "`-- ", bar: "|   ", arrow: "->" }
    } else {
        Glyphs { tee: "├─ ", last: "└─ ", bar: "│  ", arrow: "→" }
    }
}

/// The graph as trees from the documents nothing points at; a document shown before ends in (*).
fn tree(s: &Session, max: Option<usize>, gl: Glyphs) -> Vec<String> {
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

fn esc_mermaid(t: &str) -> String {
    t.replace('"', "#quot;")
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

fn mermaid(s: &Session) -> Vec<String> {
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

fn esc_dot(t: &str) -> String {
    t.replace('\\', "\\\\").replace('"', "\\\"")
}

/// C190: nodes get generated ids (`d0`, `p0`) with the text as the label, as in mermaid, so a
/// governs path spelled like a document id stays its own node.
fn dot(s: &Session) -> Vec<String> {
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
