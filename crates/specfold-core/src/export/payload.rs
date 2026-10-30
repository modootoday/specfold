//! The payload an exporter plugin asked for with `wants`: bodies, resolved links and a hash on
//! top of the compact view every plugin gets. Plugins that ask for nothing get the compact view.

use super::links;
use crate::corpus::Corpus;
use crate::graph::Graph;
use crate::markdown::frontmatter;
use crate::plugins::payload;
use crate::translation;
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

/// The documents with what `w` asks for. Bodies are the text after the frontmatter; link spans
/// are byte ranges in that text. Once the bodies sent so far pass `budget` bytes, the rest are
/// given as `body_path` (the repository-relative file) for the plugin to read itself.
pub fn docs(c: &Corpus, g: &Graph, w: Wants, budget: Option<usize>) -> Value {
    let mut used = 0usize;
    let root = c.cfg.root.clone();
    let read = |p: &str| crate::corpus::overlay::read(&root.join(p)).ok();
    let mut out = Vec::new();
    for i in 0..c.docs.len() {
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
        let m = v.as_object_mut().unwrap();
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
                o.insert("links".into(), json!(links::of_body(c, g, b)));
            }
            Value::Object(o)
        };
        let main = part(&bodies[0]);
        for (k, val) in main.as_object().unwrap() {
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
