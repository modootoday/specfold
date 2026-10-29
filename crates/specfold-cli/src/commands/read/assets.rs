//! `spec assets`: the agent assets declared under `[[assets]]`, read-only.

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
        let text = format!("{kind:<13} {:<32} {}", x.name(), x.path);
        o = o.item(json!({"kind": kind, "name": x.name(), "path": x.path, "description": description}), text);
    }
    Ok(o.next("spec check"))
}
