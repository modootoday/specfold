//! `spec assets`: the agent assets declared under `[[assets]]`; this command only reads.

use crate::cli::args::{global::Global, read::AssetsArgs};
use crate::output::Output;
use crate::session;
use anyhow::Result;
use serde_json::json;
use specfold_core::assets;

pub fn run(g: &Global, a: &AssetsArgs) -> Result<Output> {
    let s = session::open(g)?;
    let cfg = &s.corpus.cfg;
    let mut o = Output::new("assets").field("roots", cfg.raw.assets.len());
    if cfg.raw.assets.is_empty() {
        return Ok(o.next("declare roots in spec.toml: [[assets]] kind = \"skill\", path = \".agents/skills\""));
    }
    for x in assets::load(cfg) {
        let kind = x.kind.as_str();
        if a.kind.as_deref().is_some_and(|k| k != kind) {
            continue;
        }
        let description = x.field("description").unwrap_or("");
        let docs: Vec<String> = x.docs(&s.graph).into_iter().map(|i| s.corpus.docs[i].id.to_string()).collect();
        let mut text = format!("{kind:<13} {:<32} {}", x.name(), x.path);
        if !docs.is_empty() {
            text.push_str(&format!("  -> {}", docs.join(", ")));
        }
        o = o.item(
            json!({"kind": kind, "name": x.name(), "path": x.path, "description": description, "docs": docs}),
            text,
        );
    }
    Ok(o.next("spec check"))
}
