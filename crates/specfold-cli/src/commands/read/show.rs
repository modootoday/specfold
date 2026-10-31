//! `spec show`: frontmatter and outline by default; one section, the body, or the relations on
//! request, so a reader pays only for what it reads. Task items are counted so a plan's progress
//! is a number, not a guess.

use crate::cli::args::{global::Global, read::ShowArgs};
use crate::output::Output;
use crate::session;
use anyhow::{Result, bail};
use serde_json::{Value, json};
use specfold_core::markdown::{frontmatter, sections, tasks};

pub fn run(g: &Global, a: &ShowArgs) -> Result<Output> {
    let (s, doc, remote) = session::open_for(g, &a.doc)?;
    let idx = session::resolve(&s, &doc)?;
    let d = &s.corpus.docs[idx];
    let shown = remote.as_ref().map_or(d.id.to_string(), |r| format!("{r}:{}", d.id));
    let text = specfold_core::corpus::overlay::read(&s.corpus.cfg.root.join(&d.path))?;
    let split = frontmatter::split(&text);
    let body = split.body;
    let mut o = Output::new("show")
        .unpaged()
        .field("remote", remote)
        .field("id", d.id.to_string())
        .field("path", d.path.clone())
        .field("frontmatter", d.fm().cloned().map(Value::Object).unwrap_or(Value::Null))
        .head(format!("{shown}  {}", d.path));
    for (k, v) in d.fm().into_iter().flatten() {
        o = o.head(format!("  {k}: {}", if let Value::String(s) = v { s.clone() } else { v.to_string() }));
    }
    let mut cont: Option<String> = None;
    if let Some(name) = &a.section {
        let Some(sec) = sections::extract(body, name).or_else(|| {
            d.chapters
                .iter()
                .find_map(|c| sections::extract(&std::fs::read_to_string(s.corpus.cfg.root.join(c)).ok()?, name))
        }) else {
            bail!("{}", sections::not_found(body, split.body_line, name));
        };
        o = o.field("section", sec.clone()).foot(String::new()).foot(sec);
    } else if a.body {
        let (body, rest) = match g.max_bytes {
            Some(b) if body.len() > b => split_at_headings(body, b),
            _ => (body.to_string(), vec![]),
        };
        o = o.field("body", body.clone()).foot(String::new()).foot(body.trim_end().to_string());
        if let Some(first) = rest.first() {
            o = o.field("cut_by", "max_bytes").field("rest_sections", json!(rest)).foot(format!(
                "+{} more section(s) not shown: {}",
                rest.len(),
                rest.join(" | ")
            ));
            cont = Some(format!(
                "spec show {} --section '{}'",
                specfold_core::shell::word(&shown),
                first.replace('\'', "")
            ));
        }
    } else {
        let outline = sections::outline(body);
        o = o.field(
            "outline",
            json!(outline.iter().map(|(l, t)| json!({"level": l, "heading": t})).collect::<Vec<_>>()),
        );
        o = o.foot("outline:");
        for (l, t) in &outline {
            o = o.foot(format!("{}{t}", "  ".repeat(*l)));
        }
        if !d.chapters.is_empty() {
            o = o.field("chapters", json!(d.chapters)).foot(format!("chapters: {}", d.chapters.join(", ")));
        }
    }
    // Chapters belong to the document, so their items count toward its progress.
    let mut items: Vec<Value> = tasks::list(body).into_iter().map(|t| task_json(&d.path, split.body_line, t)).collect();
    for c in &d.chapters {
        let ch = std::fs::read_to_string(s.corpus.cfg.root.join(c)).unwrap_or_default();
        let cs = frontmatter::split(&ch);
        items.extend(tasks::list(cs.body).into_iter().map(|t| task_json(c, cs.body_line, t)));
    }
    let (done, total) = (items.iter().filter(|t| t["done"] == true).count(), items.len());
    if total > 0 || a.tasks {
        o = o.head(format!("  tasks: {done}/{total} done"));
    }
    o = o.field("tasks", json!({"done": done, "open": total - done, "total": total}));
    if a.tasks {
        o = o.foot("tasks:");
        for t in &items {
            let mark = if t["done"] == true { "x" } else { " " };
            o = o.foot(format!(
                "  [{mark}] {}  {}:{}",
                t["text"].as_str().unwrap_or(""),
                t["file"].as_str().unwrap_or(""),
                t["line"]
            ));
        }
        o = o.field("task_items", Value::Array(items));
    }
    if a.rel {
        o = relations(&s, idx, o);
    }
    o = assets(&s, idx, o);
    let next = if let Some(c) = cont {
        c
    } else if a.tasks && done < total {
        format!(
            "spec show {} --body   # finish the open items, then spec set {} updated=today --write",
            specfold_core::shell::word(&shown),
            specfold_core::shell::word(&d.id)
        )
    } else if a.section.is_some() || a.body {
        format!("spec graph impact {}", specfold_core::shell::word(&d.id))
    } else {
        format!("spec show {} --section <heading>", specfold_core::shell::word(&shown))
    };
    Ok(o.next(next))
}

/// `--rel`: the document's outgoing and incoming relations.
fn relations(s: &session::Session, idx: usize, mut o: Output) -> Output {
    let out_edges: Vec<Value> = s
        .graph
        .edges_from(idx)
        .map(|e| json!({"kind": e.kind, "to": s.corpus.docs[e.to].id.to_string(), "anchor": e.anchor}))
        .collect();
    let in_edges: Vec<Value> =
        s.graph.edges_to(idx).map(|e| json!({"kind": e.kind, "from": s.corpus.docs[e.from].id.to_string()})).collect();
    o = o.foot("relations:");
    for e in &out_edges {
        o = o.foot(format!("  -> {} {}", e["kind"].as_str().unwrap_or(""), e["to"].as_str().unwrap_or("")));
    }
    for e in &in_edges {
        o = o.foot(format!("  <- {} {}", e["kind"].as_str().unwrap_or(""), e["from"].as_str().unwrap_or("")));
    }
    o = o.field("outgoing", Value::Array(out_edges)).field("incoming", Value::Array(in_edges));
    o
}

/// Assets that name this document; read on every show, since none are stored in the graph.
fn assets(s: &session::Session, idx: usize, mut o: Output) -> Output {
    let named: Vec<Value> = specfold_core::assets::load(&s.corpus.cfg)
        .iter()
        .filter(|x| x.docs(&s.graph).contains(&idx))
        .map(|x| json!({"kind": x.kind.as_str(), "name": x.name(), "path": x.path}))
        .collect();
    if !named.is_empty() {
        o = o.foot("assets:");
        for x in &named {
            o = o.foot(format!("  <- {} {}", x["kind"].as_str().unwrap_or(""), x["path"].as_str().unwrap_or("")));
        }
    }
    o = o.field("assets", Value::Array(named));
    o
}

/// Lines are reported in the file, where an editor or agent will look for them.
fn task_json(file: &str, body_line: usize, t: tasks::Task) -> Value {
    json!({"file": file, "line": t.line + body_line - 1, "done": t.done, "text": t.text})
}

/// The body cut at a heading so whole sections fit in `budget` bytes (at least the first one),
/// with the headings of the sections left out. Headings inside code fences do not count.
fn split_at_headings(body: &str, budget: usize) -> (String, Vec<String>) {
    let mut starts = vec![];
    let (mut pos, mut fence) = (0, specfold_core::markdown::Fence::default());
    for line in body.split_inclusive('\n') {
        let t = line.trim_start();
        if fence.step(t) {
        } else if !fence.inside() && t.starts_with('#') && pos > 0 {
            starts.push((pos, t.trim_start_matches('#').trim().to_string()));
        }
        pos += line.len();
    }
    let cut = starts.iter().rev().find(|(p, _)| *p <= budget).or(starts.first()).map(|(p, _)| *p);
    match cut {
        Some(c) => {
            let rest = starts.iter().filter(|(p, _)| *p >= c).map(|(_, h)| h.clone()).collect();
            (body[..c].to_string(), rest)
        }
        None => (body.to_string(), vec![]),
    }
}
