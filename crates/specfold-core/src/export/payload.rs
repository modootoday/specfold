//! The payload an exporter plugin asked for with `wants`: bodies, resolved links and a hash on
//! top of the compact view every plugin gets. Plugins that ask for nothing get the compact view.

use super::links;
use crate::corpus::Corpus;
use crate::corpus::graph::Graph;
use crate::corpus::translation;
use crate::model::markdown::frontmatter;
use crate::plugins::payload;
use serde_json::{Value, json};

/// The extras a plugin can ask for in `describe`'s `wants`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Wants {
    pub body: bool,
    pub links: bool,
}

impl Wants {
    pub fn from(list: &[String]) -> Self {
        Wants { body: list.iter().any(|w| w == "body"), links: list.iter().any(|w| w == "links") }
    }

    pub fn any(&self) -> bool {
        self.body || self.links
    }
}

/// The documents in `only` with what `w` asks for; link spans are byte ranges in the body. Past
/// `budget` bytes, bodies go as `body_path` for the plugin to read. A reference to a document left
/// out is sent unresolved (`reason: "not exported"`) so nothing about it reaches the plugin.
pub fn docs(c: &Corpus, g: &Graph, w: Wants, budget: Option<usize>, only: &[usize]) -> Value {
    let mut used = 0usize;
    let root = c.cfg.spec_home.clone();
    let read = |p: &str| crate::corpus::overlay::read(&root.join(p)).ok();
    let mut out = Vec::new();
    for &i in only {
        let mut v = payload::doc(c, i);
        if !w.any() {
            out.push(v);
            continue;
        }
        let d = &c.docs[i];
        let mut parts = vec![(d.path.clone(), read(&d.path).unwrap_or_default())];
        let mut chapters = d.chapters.clone();
        chapters.sort();
        parts.extend(chapters.into_iter().map(|ch| {
            let t = read(&ch).unwrap_or_default();
            (ch, t)
        }));
        let bodies: Vec<(String, String)> =
            parts.iter().map(|(p, t)| (p.clone(), frontmatter::split(t).body.to_string())).collect();
        let parts_for_rev: Vec<translation::Part> =
            bodies.iter().map(|(p, b)| translation::Part { label: p.clone(), body: b.clone() }).collect();
        let m = v.as_object_mut().expect("payload is built as an object");
        m.insert("hash".into(), json!(translation::rev_of_parts(&parts_for_rev)));
        let size: usize = bodies.iter().map(|(_, b)| b.len()).sum();
        let inline = w.body && budget.is_none_or(|b| used + size <= b);
        if inline {
            used += size;
        }
        let part = |(p, b): &(String, String)| {
            let mut o = serde_json::Map::new();
            o.insert("path".into(), json!(p));
            if w.body {
                if inline {
                    o.insert("body".into(), json!(b));
                } else {
                    o.insert("body_path".into(), json!(p));
                }
            }
            if w.links {
                let ls: Vec<links::Link> = links::of_body(c, g, b).into_iter().map(|l| hide(l, only)).collect();
                o.insert("links".into(), json!(ls));
            }
            Value::Object(o)
        };
        let main = part(&bodies[0]);
        for (k, val) in main.as_object().expect("part() returns an object") {
            if k != "path" {
                m.insert(k.clone(), val.clone());
            }
        }
        if bodies.len() > 1 {
            m.insert("chapter_bodies".into(), Value::Array(bodies[1..].iter().map(part).collect()));
        }
        out.push(v);
    }
    Value::Array(out)
}

/// Which documents an export may show: committed ones, plus local ones with `local`.
pub fn visible(c: &Corpus, local: bool) -> Vec<usize> {
    (0..c.docs.len()).filter(|&i| local || c.docs[i].tier == crate::model::Tier::Committed).collect()
}

/// Edges between documents in `only`.
pub fn edges(c: &Corpus, g: &Graph, only: &[usize]) -> Value {
    Value::Array(
        g.edges
            .iter()
            .filter(|e| only.contains(&e.from) && only.contains(&e.to))
            .map(|e| json!({"from": c.docs[e.from].id.to_string(), "to": c.docs[e.to].id.to_string(), "kind": e.kind, "anchor": e.anchor}))
            .collect(),
    )
}

fn hide(mut l: links::Link, only: &[usize]) -> links::Link {
    if let Some(i) = l.idx
        && !only.contains(&i)
    {
        l.target_id = None;
        l.anchor = None;
        l.resolved_path = None;
        l.title = None;
        l.status = None;
        l.idx = None;
        l.reason = Some("not exported".into());
    }
    l
}

/// C252: every file of the exported documents must read as text before anything is exported;
/// one that does not stops the export (exit 3) naming it, instead of going out as an empty body.
pub fn readable(c: &Corpus, only: &[usize]) -> anyhow::Result<()> {
    for &i in only {
        let d = &c.docs[i];
        let mut files = vec![d.path.clone()];
        files.extend(d.chapters.iter().cloned());
        for p in files {
            if let Err(e) = crate::corpus::overlay::read(&c.cfg.spec_home.join(&p)) {
                return Err(anyhow::Error::new(std::io::Error::new(
                    e.kind(),
                    format!(
                        "cannot export {p} ({e}): nothing was written; convert it from its encoding (iconv -f <encoding> -t UTF-8); next: spec explain S1"
                    ),
                )));
            }
        }
    }
    Ok(())
}
