//! Audit items contributed by plugins that provide `audits`.

use super::types::{Item, Kind};
use crate::corpus::Corpus;
use crate::graph::Graph;
use crate::plugins::{self, payload, process};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
struct PluginItem {
    subject: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    line: usize,
    detail: String,
    next: Option<String>,
}

#[derive(Deserialize)]
struct Response {
    #[serde(default)]
    items: Vec<PluginItem>,
}

pub fn run(c: &Corpus, g: &Graph, out: &mut Vec<Item>) {
    for p in plugins::load(&c.cfg) {
        let Ok(d) = &p.description else { continue };
        if d.provides.audits.is_empty() {
            continue;
        }
        let params =
            json!({"audits": d.provides.audits, "docs": payload::docs(c, None), "edges": payload::edges(c, g)});
        match process::call::<Response>(&c.cfg, &p.def, "audit.run", params) {
            Ok(r) => out.extend(r.items.into_iter().map(|x| Item {
                kind: Kind::Plugin,
                subject: x.subject,
                path: x.path,
                line: x.line,
                detail: format!("[{}] {}", p.def.name, x.detail),
                next: x.next,
            })),
            Err(e) => out.push(Item {
                kind: Kind::Plugin,
                subject: p.def.name.clone(),
                path: "spec.toml".into(),
                line: 0,
                detail: e.to_string(),
                next: None,
            }),
        }
    }
}
