//! `spec new`.

use super::preview;
use crate::cli::args::{global::Global, write::NewArgs};
use crate::output::Output;
use crate::session;
use anyhow::{Context, Result, bail};
use serde_json::json;
use specfold_core::assets;
use specfold_core::authoring::new::{self, NewDoc};
use specfold_core::config::types::AssetKind;
use specfold_core::model::stamp::slugify;

pub fn run(g: &Global, a: &NewArgs) -> Result<Output> {
    // CLI-1: from 0.4 `new` previews unless --write; 0.3 still writes, and says so on stderr.
    if !a.dry_run && !a.write {
        eprintln!(
            "warning: spec new writes without --write in 0.3; from 0.4 it previews unless --write. Add --write to keep writing (--dry-run previews)."
        );
    }
    let s = session::open(g)?;
    if let Some(lang) = &a.translation {
        return translation(&s, a, lang);
    }
    let ty = a.ty.clone().unwrap_or_default();
    if let Some(kind) = asset_kind(&s, &ty) {
        return asset(&s, a, kind);
    }
    let governs: Vec<String> = a.governs.iter().map(|p| session::repo_path(&s, p)).collect();
    let req = NewDoc {
        ty: ty.clone(),
        title: a.title().to_string(),
        governs,
        owner: a.owner.clone(),
        slug: a.slug.clone(),
        from: a.from.clone(),
        local: a.local,
        bundle: a.bundle,
        key: None,
        body: None,
        depends_on: a.depends_on.clone(),
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
    let o = match &created.slug_note {
        Some(note) => o.field("slug_note", note.clone()).head(format!("warn  {note}")),
        None => o,
    };
    // The same title twice is usually a rerun; say so before a second copy is written.
    let same: Vec<String> = s
        .corpus
        .docs
        .iter()
        .filter(|d| {
            d.id.ty == ty && d.str_field("title").is_some_and(|t| t.trim().eq_ignore_ascii_case(a.title().trim()))
        })
        .map(|d| d.id.to_string())
        .collect();
    let o = if same.is_empty() {
        o
    } else {
        o.field("same_title", json!(same)).head(format!("warn  same title as {}", same.join(", ")))
    };
    let mut o = preview::finish(o, &plan, &s.corpus.cfg.root, !a.dry_run, "rerun without --dry-run")?;
    if a.local
        && !a.dry_run
        && let Some(note) = ignore_local(&s.corpus.cfg.root, &created.path, s.corpus.cfg.local_dir())
    {
        o = o.field("ignored", note.clone()).foot(note);
    }
    // A slug taken from a few ASCII letters of a mostly non-ASCII title rarely names the
    // document: the next step is to name it.
    Ok(match (&created.slug_note, a.dry_run) {
        (Some(_), false) => o.next(format!("spec mv {} --slug <kebab-case-name> --write", created.id)),
        (Some(_), true) => o.next("rerun with --slug <kebab-case-name>"),
        (None, true) => o,
        (None, false) => o.next(format!("spec show {}", created.id)),
    })
}

/// `spec new --translation <lang> --from <source>`.
fn translation(s: &session::Session, a: &NewArgs, lang: &str) -> Result<Output> {
    let from = a.from.as_deref().unwrap_or_default();
    let title = a.title.as_deref().or(a.title_text.as_deref());
    let (created, plan) = new::translation(&s.corpus, &s.graph, from, lang, title)?;
    if let Some(ty) = &a.ty
        && !created.id.starts_with(&format!("{ty}:"))
    {
        bail!("--from {from} is not a {ty}; leave out the type to translate it as it is");
    }
    let o = Output::new("new")
        .unpaged()
        .field("id", created.id.clone())
        .field("path", created.path.clone())
        .field("owner_reason", created.owner_reason.clone())
        .head(format!("{} {}  {}", if a.dry_run { "would create" } else { "created" }, created.id, created.path))
        .head(created.owner_reason.clone())
        .head("the body is the source's: translate it; source_rev already records the source it was copied from");
    let o = preview::finish(o, &plan, &s.corpus.cfg.root, !a.dry_run, "rerun without --dry-run")?;
    Ok(if a.dry_run { o } else { o.next(format!("spec show {}", created.id)) })
}

/// The local directory is meant to stay out of git; the first local document makes sure it does,
/// with a line in .git/info/exclude (which is never committed) when nothing ignores it yet.
fn ignore_local(root: &std::path::Path, path: &str, local_dir: &str) -> Option<String> {
    let git = |args: &[&str]| std::process::Command::new("git").arg("-C").arg(root).args(args).output().ok();
    let checked = git(&["check-ignore", "-q", "--no-index", path])?;
    if checked.status.code() != Some(1) {
        return None; // ignored already, or not a git repository
    }
    let out = git(&["rev-parse", "--git-path", "info/exclude"])?;
    let rel = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() || rel.is_empty() {
        return None;
    }
    let file = root.join(&rel);
    let name = local_dir.trim_end_matches('/').rsplit('/').next().unwrap_or(local_dir);
    let mut text = std::fs::read_to_string(&file).unwrap_or_default();
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(&format!("# local documents (spec new --local)\n{name}/\n"));
    std::fs::create_dir_all(file.parent()?).ok()?;
    std::fs::write(&file, text).ok()?;
    Some(format!("{name}/ was not ignored by git; added it to {rel}"))
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
    let name = a.slug.clone().unwrap_or_else(|| slugify(a.title()));
    let description = a.description.clone().unwrap_or_else(|| a.title().to_string());
    let cfg = &s.corpus.cfg;
    let Some((path, text, covered)) = assets::template(cfg, kind, &name, &description, a.title()) else {
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
