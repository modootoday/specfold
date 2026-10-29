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
    let s = session::open(g)?;
    let idx = session::resolve(&s, &a.doc)?;
    let d = &s.corpus.docs[idx];
    let text = std::fs::read_to_string(s.corpus.cfg.root.join(&d.path))?;
    let split = frontmatter::split(&text);
    let body = split.body;
    let mut o = Output::new("show")
        .unpaged()
        .field("id", d.id.to_string())
        .field("path", d.path.clone())
        .field("frontmatter", d.fm().cloned().map(Value::Object).unwrap_or(Value::Null))
        .head(format!("{}  {}", d.id, d.path));
    for (k, v) in d.fm().into_iter().flatten() {
        o = o.head(format!("  {k}: {}", if let Value::String(s) = v { s.clone() } else { v.to_string() }));
    }
    if let Some(name) = &a.section {
        let Some(sec) = sections::extract(body, name).or_else(|| {
            d.chapters
                .iter()
                .find_map(|c| sections::extract(&std::fs::read_to_string(s.corpus.cfg.root.join(c)).ok()?, name))
        }) else {
            let have: Vec<String> = sections::outline(body).into_iter().map(|(_, t)| t).collect();
            bail!("no section '{name}' (sections: {})", have.join(" | "));
        };
        o = o.field("section", sec.clone()).foot(String::new()).foot(sec);
    } else if a.body {
        o = o.field("body", body.to_string()).foot(String::new()).foot(body.trim_end().to_string());
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
        let out_edges: Vec<Value> = s
            .graph
            .edges_from(idx)
            .map(|e| json!({"kind": e.kind, "to": s.corpus.docs[e.to].id.to_string(), "anchor": e.anchor}))
            .collect();
        let in_edges: Vec<Value> = s
            .graph
            .edges_to(idx)
            .map(|e| json!({"kind": e.kind, "from": s.corpus.docs[e.from].id.to_string()}))
            .collect();
        o = o.foot("relations:");
        for e in &out_edges {
            o = o.foot(format!("  -> {} {}", e["kind"].as_str().unwrap_or(""), e["to"].as_str().unwrap_or("")));
        }
        for e in &in_edges {
            o = o.foot(format!("  <- {} {}", e["kind"].as_str().unwrap_or(""), e["from"].as_str().unwrap_or("")));
        }
        o = o.field("outgoing", Value::Array(out_edges)).field("incoming", Value::Array(in_edges));
    }
    // Assets that name this document; read on every show, since none are stored in the graph.
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
    let next = if a.tasks && done < total {
        format!("spec show {} --body   # finish the open items, then spec set {} updated=today", d.id, d.id)
    } else if a.section.is_some() || a.body {
        format!("spec graph impact {}", d.id)
    } else {
        format!("spec show {} --section <heading>", d.id)
    };
    Ok(o.next(next))
}

/// Lines are reported in the file, where an editor or agent will look for them.
fn task_json(file: &str, body_line: usize, t: tasks::Task) -> Value {
    json!({"file": file, "line": t.line + body_line - 1, "done": t.done, "text": t.text})
}
