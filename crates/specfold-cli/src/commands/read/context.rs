//! `spec context`: the documents that govern the code you are about to change.

use crate::cli::args::{global::Global, read::ContextArgs};
use crate::output::Output;
use crate::session;
use anyhow::Result;
use serde_json::json;
use specfold_core::graph::query;
use specfold_core::model::lifecycle;
use specfold_core::remotes;
use std::collections::BTreeSet;

pub fn run(g: &Global, a: &ContextArgs) -> Result<Output> {
    let s = session::open(g)?;
    let mut o = Output::new("context");
    let mut seen = BTreeSet::new();
    let mut first: Option<String> = None;
    for p in &a.paths {
        let rel = s.loc.governed_path(&s.corpus.cfg, p)?;
        if let Some((name, inner)) = specfold_core::workspace::projects::submodule_at(&s.corpus.cfg, &rel) {
            o = remote(o, &s.corpus.cfg, &rel, &name, &inner, a.all);
            continue;
        }
        let owner = s.corpus.owners.containing(&rel);
        o = o.head(format!("{rel}  (owner {})", owner.id));
        for gv in query::governing(&s.corpus, &rel) {
            let d = &s.corpus.docs[gv.idx];
            let status = d.status().unwrap_or_default();
            if (!a.all && !lifecycle::is_current(&status)) || !seen.insert(gv.idx) {
                continue;
            }
            let verify = d.list_field("verify");
            first.get_or_insert_with(|| d.id.to_string());
            let mut text = format!("  {:<34} {:<9} {}", d.id.to_string(), status, d.title.clone().unwrap_or_default());
            if !verify.is_empty() {
                text.push_str(&format!("\n      verify: {}", verify.join(" && ")));
            }
            o = o.item(
                json!({"id": d.id.to_string(), "title": d.title, "status": status, "path": d.path, "path_queried": rel, "governs": gv.pattern, "verify": verify, "anchors": d.anchors.len()}),
                text,
            );
        }
    }
    if o.items.is_empty() {
        o = o.foot("no document governs these paths");
        let p = a.paths.first().cloned().unwrap_or_default();
        return Ok(o.next(format!(
            "spec new sot --governs {} --title '…' --write",
            specfold_core::shell::word(specfold_core::workspace::escape_glob(&p))
        )));
    }
    if first.is_none() {
        return Ok(o);
    }
    // C172: only a sot has a `rule` section; any other type is shown whole.
    let id = first.unwrap_or_default();
    let next = if id.starts_with("sot:") {
        format!("spec show {} --section rule", specfold_core::shell::word(&id))
    } else {
        format!("spec show {}", specfold_core::shell::word(&id))
    };
    Ok(o.next(next))
}

/// A1: a path inside a submodule registered as a remote is owned there; its documents are read,
/// never written, from that checkout.
fn remote(mut o: Output, cfg: &specfold_core::config::Config, rel: &str, name: &str, inner: &str, all: bool) -> Output {
    let Some(l) = remotes::load(cfg, name) else {
        return o.head(format!("{rel}  (remote {name}, not checked out; next: git submodule update --init)"));
    };
    let owner = l.corpus.owners.containing(inner);
    o = o.head(format!("{rel}  (owner {name}:{}, read-only remote; write inside the submodule)", owner.id));
    for gv in query::governing(&l.corpus, inner) {
        let d = &l.corpus.docs[gv.idx];
        let status = d.status().unwrap_or_default();
        if !all && !lifecycle::is_current(&status) {
            continue;
        }
        let id = format!("{name}:{}", d.id);
        let text = format!("  {:<34} {:<9} {}", id, status, d.title.clone().unwrap_or_default());
        o = o.item(
            json!({"id": id, "title": d.title, "status": status, "path": format!("{}/{}", cfg.raw.remotes.get(name).map(|r| r.path.as_str()).unwrap_or(""), d.path), "path_queried": rel, "governs": gv.pattern, "remote": name}),
            text,
        );
    }
    o
}
