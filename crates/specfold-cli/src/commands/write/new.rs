//! `spec new`.

use super::preview;
use crate::cli::args::{global::Global, write::NewArgs};
use crate::output::Output;
use crate::session;
use anyhow::{Context, Result, bail};
use specfold_core::assets;
use specfold_core::authoring::new::{self, NewDoc};
use specfold_core::config::types::AssetKind;
use specfold_core::model::stamp::slugify;

pub fn run(g: &Global, a: &NewArgs) -> Result<Output> {
    let s = session::open(g)?;
    if let Some(kind) = asset_kind(&s, &a.ty) {
        return asset(&s, a, kind);
    }
    let governs: Vec<String> = a.governs.iter().map(|p| session::repo_path(&s, p)).collect();
    let req = NewDoc {
        ty: a.ty.clone(),
        title: a.title.clone(),
        governs,
        owner: a.owner.clone(),
        slug: a.slug.clone(),
        from: a.from.clone(),
        local: a.local,
        bundle: a.bundle,
        key: None,
        body: None,
    };
    let (created, plan) = new::plan(&s.corpus, &s.graph, &req)?;
    let o = Output::new("new")
        .unpaged()
        .field("id", created.id.clone())
        .field("path", created.path.clone())
        .field("owner_reason", created.owner_reason.clone())
        .head(format!("{} {}  {}", if a.dry_run { "would create" } else { "created" }, created.id, created.path))
        .head(format!("owner: {}", created.owner_reason))
        .head("dates and timestamp keys are UTC");
    let o = preview::finish(o, &plan, &s.corpus.cfg.root, !a.dry_run, "rerun without --dry-run")?;
    Ok(if a.dry_run { o } else { o.next(format!("spec show {}", created.id)) })
}

/// skill, rule and agent make agent assets unless a document type of that name is declared.
fn asset_kind(s: &session::Session, ty: &str) -> Option<AssetKind> {
    if s.corpus.cfg.registry.get(ty).is_some() {
        return None;
    }
    match ty {
        "skill" => Some(AssetKind::Skill),
        "rule" => Some(AssetKind::Rule),
        "agent" => Some(AssetKind::Agent),
        _ => None,
    }
}

fn asset(s: &session::Session, a: &NewArgs, kind: AssetKind) -> Result<Output> {
    let name = a.slug.clone().unwrap_or_else(|| slugify(&a.title));
    let description = a.description.clone().unwrap_or_else(|| a.title.clone());
    let cfg = &s.corpus.cfg;
    let Some((path, text, covered)) = assets::template(cfg, kind, &name, &description, &a.title) else {
        bail!("no template for {}", kind.as_str());
    };
    let full = cfg.root.join(&path);
    if full.exists() {
        bail!("{path} already exists; edit it instead");
    }
    let mut o = Output::new("new")
        .unpaged()
        .field("kind", kind.as_str())
        .field("name", name.clone())
        .field("declared", covered)
        .field("path", path.clone())
        .head(format!("{} {} {name}  {path}", if a.dry_run { "would create" } else { "created" }, kind.as_str()))
        .head("agent assets carry no id");
    if !covered {
        o = o.head(format!(
            "not declared in spec.toml, so check skips it: add [[assets]] kind = \"{}\" path = \"{}\"",
            kind.as_str(),
            declare_path(kind, &path)
        ));
    }
    if a.dry_run {
        return Ok(o.field("text", text).next("rerun without --dry-run"));
    }
    if let Some(dir) = full.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }
    std::fs::write(&full, text).with_context(|| format!("cannot write {path}"))?;
    Ok(o.next("spec check --rule A1"))
}

/// The directory to declare for a new asset path.
fn declare_path(kind: AssetKind, path: &str) -> String {
    let up = if kind == AssetKind::Skill { 2 } else { 1 };
    let mut p = path;
    for _ in 0..up {
        p = p.rsplit_once('/').map_or("", |(d, _)| d);
    }
    p.to_string()
}
