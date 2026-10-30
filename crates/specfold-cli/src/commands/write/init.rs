//! `spec init`: a starter spec.toml.

use crate::cli::args::{global::Global, write::InitArgs};
use crate::output::Output;
use crate::session;
use anyhow::{Result, bail};
use specfold_core::config::starter::{self, StarterKind};
use specfold_core::config::{defaults, upgrade};

fn parse_kind(s: &str) -> Result<StarterKind> {
    let (body, manifest) = match s.strip_suffix(":manifest") {
        Some(b) => (b, true),
        None => (s, false),
    };
    let Some((pattern, id)) = body.split_once('=') else {
        bail!("--owner-kind '{s}' is not pattern=id-template, e.g. 'services/*={{1}}'")
    };
    Ok(StarterKind { pattern: pattern.trim().into(), id: id.trim().into(), manifest })
}

/// `spec init --upgrade`: reads the file as text, since an old file may not parse.
fn upgrade(g: &Global, a: &InitArgs) -> Result<Output> {
    let mut dir = match &g.dir {
        Some(d) => d.canonicalize()?,
        None => std::env::current_dir()?,
    };
    let path = loop {
        let p = dir.join("spec.toml");
        if p.is_file() {
            break p;
        }
        if !dir.pop() {
            bail!("no spec.toml here or above; create one: spec init --write");
        }
    };
    let text = std::fs::read_to_string(&path)?;
    let (new, changes) = upgrade::plan(&text, upgrade::RENAMED);
    let mut o =
        Output::new("init").unpaged().field("path", path.display().to_string()).field("changes", changes.clone());
    if changes.is_empty() {
        return Ok(o
            .head(format!("{} is current (schema {})", path.display(), defaults::schema()))
            .next("spec doctor"));
    }
    for c in &changes {
        o = o.head(c.clone());
    }
    if a.write {
        std::fs::write(&path, new)?;
        Ok(o.field("written", true).foot("written").next("spec doctor"))
    } else {
        Ok(o.field("written", false).next("spec init --upgrade --write"))
    }
}

pub fn run(g: &Global, a: &InitArgs) -> Result<Output> {
    if a.upgrade {
        return upgrade(g, a);
    }
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
