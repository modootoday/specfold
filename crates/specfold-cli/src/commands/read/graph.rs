//! `spec graph`: the whole graph as a tree, Mermaid or DOT; impact, chain and governs.

use crate::cli::args::{global::Global, read::GraphArgs};
use crate::output::Output;
use crate::session::{self, Session};
use anyhow::{Result, bail};
use serde_json::json;
use specfold_core::graph::query;
use std::collections::{BTreeSet, VecDeque};

pub fn run(g: &Global, a: &GraphArgs) -> Result<Output> {
    let s = session::open(g)?;
    let c = &s.corpus;
    let id = |i: usize| c.docs[i].id.to_string();
    let mut o = Output::new("graph");
    if a.query.is_some() && a.format != "tree" {
        bail!("--format {} draws the whole graph: spec graph --format {}", a.format, a.format);
    }
    match (a.query.as_deref(), a.target.as_deref()) {
        (None, _) => {
            o = o.field("nodes", json!(c.docs.iter().map(|d| json!({"id": d.id.to_string(), "type": d.id.ty, "owner": d.id.owner, "status": d.status(), "path": d.path, "governs": governs(&s, d)})).collect::<Vec<_>>()));
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
                .flat_map(|d| governs(&s, d).into_iter().map(move |p| json!({"from": d.id.to_string(), "path": p})))
                .collect();
            o = o.field("governs", json!(gov)).unpaged();
            let lines = match a.format.as_str() {
                "mermaid" => mermaid(&s),
                "dot" => dot(&s),
                _ => {
                    let mut l = vec![format!("{} documents, {} edges", c.docs.len(), s.graph.edges.len())];
                    l.extend(tree(&s, a.depth, glyphs(a.ascii)));
                    return Ok(l.into_iter().fold(o, |o, x| o.head(x)).next("spec graph impact <id>"));
                }
            };
            o = o.field("diagram", lines.join("\n"));
            Ok(lines.into_iter().fold(o, |o, x| o.head(x)))
        }
        (Some("impact"), Some(t)) => {
            let idx = session::resolve(&s, t)?;
            let arrow = glyphs(a.ascii).arrow;
            for r in reach(&s, idx, a.depth) {
                o = o.item(
                    json!({"id": id(r.idx), "depth": r.depth, "kind": r.kind, "via": id(r.via), "status": c.docs[r.idx].status()}),
                    format!("{}{}  {} {arrow} {}  ({})", "  ".repeat(r.depth), id(r.idx), r.kind, id(r.via), r.depth),
                );
            }
            Ok(o.head(format!("documents that reach {} (relation, distance):", id(idx)))
                .next(format!("spec show {} --rel", id(idx))))
        }
        (Some("chain"), Some(t)) => {
            let idx = session::resolve(&s, t)?;
            let mut chain = older(&s, idx);
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
fn glyphs(ascii: bool) -> Glyphs {
    let locale = ["LC_ALL", "LC_CTYPE", "LANG"].iter().find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()));
    let utf8 = locale.is_none_or(|l| {
        let l = l.to_ascii_lowercase();
        l.contains("utf-8") || l.contains("utf8")
    });
    if ascii || !utf8 {
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

fn dot(s: &Session) -> Vec<String> {
    let c = &s.corpus;
    let mut l = vec!["digraph specfold {".to_string(), "  rankdir=LR;".to_string(), "  node [shape=box];".to_string()];
    for d in &c.docs {
        l.push(format!("  \"{}\";", esc_dot(&d.id.to_string())));
    }
    for e in &s.graph.edges {
        let (f, t) = (c.docs[e.from].id.to_string(), c.docs[e.to].id.to_string());
        l.push(format!("  \"{}\" -> \"{}\" [label=\"{}\"];", esc_dot(&f), esc_dot(&t), esc_dot(&e.kind)));
    }
    let mut paths = BTreeSet::new();
    for d in &c.docs {
        for p in governs(s, d) {
            if paths.insert(p.clone()) {
                l.push(format!("  \"{}\" [shape=folder];", esc_dot(&p)));
            }
            l.push(format!(
                "  \"{}\" -> \"{}\" [label=\"governs\", style=dashed];",
                esc_dot(&d.id.to_string()),
                esc_dot(&p)
            ));
        }
    }
    l.push("}".to_string());
    l
}
