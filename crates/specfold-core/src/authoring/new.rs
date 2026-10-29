//! Creating a document: compute its owner and path, assign its key, write its frontmatter and
//! body skeleton.

use super::plan::Plan;
use super::set;
use crate::corpus::Corpus;
use crate::graph::{Graph, Resolution};
use crate::markdown::frontmatter::{edit, render};
use crate::model::doctype::KeyKind;
use crate::model::{DocId, Tier, stamp};
use crate::placement::{self, paths};
use anyhow::{Result, anyhow, bail};
use serde::Serialize;
use serde_json::{Map, Value, json};

#[derive(Debug, Clone, Default)]
pub struct NewDoc {
    pub ty: String,
    pub title: String,
    pub governs: Vec<String>,
    /// Overrides the computed owner (refused if it breaks placement, unless local).
    pub owner: Option<String>,
    /// For slug types; defaults to the slugified title.
    pub slug: Option<String>,
    /// Supersede this document.
    pub from: Option<String>,
    pub local: bool,
    /// Force a bundle directory even for a single-file type.
    pub bundle: bool,
    /// Fixed key, for imports; otherwise now.
    pub key: Option<String>,
    /// Body to write instead of the type's template (audit records, generated documents).
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Created {
    pub id: String,
    pub path: String,
    pub owner_reason: String,
    /// Why the slug is not what the title reads as, when it drops most of a non-ASCII title.
    pub slug_note: Option<String>,
}

/// A slug from the title. Titles in other scripts lose their words to slugify, so when less than
/// half the title's letters survive, the governed path's last segment names the slug instead,
/// and the caller is told; with neither, --slug is required.
fn derive_slug(title: &str, governs: &[String]) -> Result<(String, Option<String>)> {
    let from_title = stamp::slugify(title);
    let letters = title.chars().filter(|c| c.is_alphanumeric()).count();
    let kept = title.chars().filter(|c| c.is_ascii_alphanumeric()).count();
    if letters == 0 || kept * 2 >= letters {
        if !stamp::is_slug(&from_title) {
            bail!("cannot derive a slug from the title; pass --slug <kebab-case>");
        }
        return Ok((from_title, None));
    }
    let from_path = governs
        .iter()
        .filter_map(|g| g.trim_end_matches('/').rsplit('/').next())
        .map(|seg| stamp::slugify(seg.split('.').next().unwrap_or(seg)))
        .find(|s| stamp::is_slug(s));
    match (from_path, stamp::is_slug(&from_title)) {
        (Some(p), _) => {
            let note = format!(
                "the title is mostly non-ASCII, so the slug '{p}' comes from the governed path; pass --slug to name it"
            );
            Ok((p, Some(note)))
        }
        (None, true) => {
            let note =
                format!("the title is mostly non-ASCII, so the slug keeps only '{from_title}'; pass --slug to name it");
            Ok((from_title, Some(note)))
        }
        (None, false) => bail!(
            "the title has no ASCII words to make a slug from; give an English kebab-case --slug (replace my-slug); rerun with --slug my-slug"
        ),
    }
}

pub fn plan(c: &Corpus, g: &Graph, req: &NewDoc) -> Result<(Created, Plan)> {
    let ty = c
        .cfg
        .registry
        .get(&req.ty)
        .ok_or_else(|| anyhow!("unknown type '{}' (have: {})", req.ty, c.cfg.registry.names().join(", ")))?;
    if req.title.trim().is_empty() {
        bail!("--title is required");
    }
    let root_owner = c.owners.root();
    let decision = placement::decide(&c.owners, &req.governs, root_owner);
    let (owner, reason) = match &req.owner {
        Some(o) => {
            let ow = c.owners.by_id(o).or_else(|| c.owners.by_path(o)).ok_or_else(|| anyhow!("no owner '{o}'"))?;
            if !req.local && !req.governs.is_empty() && ow.id != decision.owner.id {
                bail!(
                    "--owner {} contradicts governs: the document belongs to {} ({})",
                    ow.id,
                    decision.owner.id,
                    decision.reason
                );
            }
            (ow, format!("owner given as {}", ow.id))
        }
        None => (decision.owner, decision.reason.clone()),
    };
    let (slug, slug_note) = match &req.slug {
        Some(s) => (s.clone(), None),
        None => derive_slug(&req.title, &req.governs)?,
    };
    if !stamp::is_slug(&slug) {
        bail!("'{slug}' is not a kebab-case slug; pass --slug in lowercase words joined by '-'");
    }
    let mut key = match ty.key {
        KeyKind::Slug => slug.clone(),
        KeyKind::Stamp => req.key.clone().unwrap_or_else(stamp::now_stamp),
    };
    while g.by_id.contains_key(&DocId::new(&ty.name, &owner.id, &key).to_string()) {
        match ty.key {
            KeyKind::Slug => bail!("{}:{}/{key} already exists; choose another --slug", ty.name, owner.id),
            KeyKind::Stamp => key = stamp::next_stamp(&key).ok_or_else(|| anyhow!("bad stamp {key}"))?,
        }
    }
    let id = DocId::new(&ty.name, &owner.id, &key);
    let tier = if req.local { Tier::Local } else { Tier::Committed };
    let dir = paths::type_dir(&c.cfg, &c.owners, owner, tier, ty);
    let path = paths::main_file(&dir, ty, &paths::stem(ty, &key, &slug), req.bundle);
    let hint = if req.local { "change [forbid] paths or the type's directory" } else { "rerun with --local" };
    super::forbid::refuse(c, &path, hint)?;
    let today = stamp::now_value();
    let mut fm = Map::new();
    fm.insert("id".into(), Value::String(id.to_string()));
    fm.insert("title".into(), Value::String(req.title.trim().to_string()));
    fm.insert("status".into(), Value::String(ty.initial.clone()));
    fm.insert("created".into(), Value::String(today.clone()));
    fm.insert("updated".into(), Value::String(today));
    let mut rel = Map::new();
    if !req.governs.is_empty() {
        rel.insert("governs".into(), json!(req.governs));
    }
    let mut plan = Plan::default();
    if let Some(from) = &req.from {
        let Resolution::Doc { idx, .. } = g.resolve(from) else { bail!("--from {from} names no single document") };
        let old = &c.docs[idx];
        if old.id.ty != ty.name {
            bail!("--from {} is a {}, not a {}", old.id, old.id.ty, ty.name);
        }
        rel.insert("supersedes".into(), json!([old.id.to_string()]));
        let old_plan = set::plan_superseded_by_new(c, idx)?;
        plan.changes.extend(old_plan.changes);
    }
    if !rel.is_empty() {
        fm.insert("rel".into(), Value::Object(rel));
    }
    let body = format!("\n# {}\n\n{}", req.title.trim(), req.body.as_deref().unwrap_or(&ty.template));
    plan.write(&path, format!("{}{body}", render(&edit::ordered(fm))), true);
    Ok((Created { id: id.to_string(), path, owner_reason: reason, slug_note }, plan))
}
