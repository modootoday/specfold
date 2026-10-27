//! `spec init`: a starter spec.toml.

use crate::cli::args::{global::Global, write::InitArgs};
use crate::output::Output;
use crate::session;
use anyhow::{Result, bail};
use specfold_core::config::starter::{self, StarterKind};

fn parse_kind(s: &str) -> Result<StarterKind> {
    let (body, manifest) = match s.strip_suffix(":manifest") {
        Some(b) => (b, true),
        None => (s, false),
    };
    let Some((pattern, id)) = body.split_once('=') else {
        bail!("--owner '{s}' is not pattern=id-template, e.g. 'apps/*={{1}}'")
    };
    Ok(StarterKind { pattern: pattern.trim().into(), id: id.trim().into(), manifest })
}

pub fn run(g: &Global, a: &InitArgs) -> Result<Output> {
    let cfg = session::config(g)?;
    if !cfg.implicit {
        bail!("{} already has a spec.toml", cfg.root.display());
    }
    let kinds = a.owners.iter().map(|k| parse_kind(k)).collect::<Result<Vec<_>>>()?;
    let text = starter::render(&a.root, &kinds);
    let path = cfg.root.join("spec.toml");
    let mut o = Output::new("init")
        .unpaged()
        .field("path", path.display().to_string())
        .field("content", text.clone())
        .head(format!("spec.toml for {}", cfg.root.display()))
        .foot(text);
    if a.write {
        std::fs::write(&path, o.summary["content"].as_str().unwrap_or_default())?;
        o = o.foot("written").next("spec new sot --governs <path> --title '…'");
    } else {
        o = o.next("spec init --write");
    }
    Ok(o)
}
