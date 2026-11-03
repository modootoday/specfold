//! The front matter a target reads, written for each page.

use super::page::Page;
use super::types::Target;
use crate::corpus::Corpus;
use serde_json::Value;
use std::collections::HashMap;

fn yaml_str(s: &str) -> String {
    serde_json::to_string(s).expect("serializing a str cannot fail")
}

fn yaml_list(v: &[String]) -> String {
    format!("[{}]", v.iter().map(|s| yaml_str(s)).collect::<Vec<_>>().join(", "))
}

fn strings(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(a)) => a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        _ => vec![],
    }
}

/// Lookups shared by every exported page.
pub(super) struct Index<'a> {
    pub(super) pages: &'a [Page],
    pub(super) by_idx: &'a HashMap<usize, usize>,
    pub(super) by_id: &'a HashMap<String, usize>,
}

pub(super) fn front(c: &Corpus, p: &Page, t: Target, ix: &Index) -> String {
    let d = &c.docs[p.idx];
    let fm = d.fm();
    let get = |k: &str| fm.and_then(|m| m.get(k));
    let mut lines = vec!["---".to_string()];
    if t == Target::Docusaurus {
        let stem = p.file.rsplit('/').next().expect("rsplit yields at least one item").trim_end_matches(".md");
        lines.push(format!("id: {}", yaml_str(stem)));
        lines.push(format!("specfold_id: {}", yaml_str(&d.id.to_string())));
        lines.push(format!("sidebar_label: {}", yaml_str(&p.title)));
        lines.push(format!("sidebar_position: {}", ix.by_idx[&p.idx] + 1));
    } else {
        lines.push(format!("id: {}", yaml_str(&d.id.to_string())));
    }
    lines.push(format!("type: {}", yaml_str(&d.id.ty)));
    if let Some(s) = d.status() {
        lines.push(format!("status: {}", yaml_str(&s)));
    }
    lines.push(format!("title: {}", yaml_str(&p.title)));
    for k in ["aliases", "tags"] {
        let v = strings(get(k));
        if !v.is_empty() {
            lines.push(format!("{k}: {}", yaml_list(&v)));
        }
    }
    for k in ["created", "updated"] {
        if let Some(v) = get(k).and_then(|v| v.as_str()) {
            lines.push(format!("{k}: {}", yaml_str(v)));
        }
    }
    if let Some(Value::Object(rel)) = get("rel") {
        for (kind, v) in rel {
            let ids: Vec<String> = strings(Some(v))
                .into_iter()
                .map(|s| match (t, g_target(&s, ix)) {
                    (Target::Obsidian, Some(q)) => format!("[[{}|{}]]", q.file.trim_end_matches(".md"), q.title),
                    _ => s,
                })
                .collect();
            if !ids.is_empty() {
                lines.push(format!("rel_{}: {}", kind.replace(['-', '.'], "_"), yaml_list(&ids)));
            }
        }
    }
    lines.push("---".into());
    lines.join("\n") + "\n\n"
}

/// The exported page for a relation target written as an id.
fn g_target<'a>(id: &str, ix: &Index<'a>) -> Option<&'a Page> {
    let target = crate::model::RefText::parse(id).target;
    let idx = ix.by_id.get(target.as_str())?;
    ix.by_idx.get(idx).map(|&i| &ix.pages[i])
}
