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
    /// Documents this one depends on (`rel.depends_on`), e.g. what a change note updates.
    pub depends_on: Vec<String>,
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
        // Mostly ASCII, but words in another script still vanish from the slug: say which.
        let dropped: Vec<&str> =
            title.split_whitespace().filter(|w| w.chars().any(|c| c.is_alphanumeric() && !c.is_ascii())).collect();
        let note = (!dropped.is_empty())
            .then(|| format!("the slug '{from_title}' leaves out '{}'; pass --slug to name it", dropped.join(" ")));
        return Ok((from_title, note));
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
    // C185: a governs value naming an existing path that holds glob characters is that path, so
    // it is written escaped; read as a glob it would name other files or none.
    let mut owned = req.clone();
    for g in &mut owned.governs {
        if g.contains(['[', '*', '?', '{']) && c.cfg.root.join(g.as_str()).exists() {
            *g = crate::workspace::escape_glob(g);
        }
    }
    let req = &owned;
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
    if !req.depends_on.is_empty() {
        let mut ids = vec![];
        for r in &req.depends_on {
            let Resolution::Doc { idx, .. } = g.resolve(r) else { bail!("--depends-on {r} names no single document") };
            ids.push(c.docs[idx].id.to_string());
        }
        rel.insert("depends_on".into(), json!(ids));
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
    let template = with_required_sections(&ty.template, &ty.sections);
    let body = format!("\n# {}\n\n{}", req.title.trim(), req.body.as_deref().unwrap_or(&template));
    plan.write(&path, format!("{}{body}", render(&edit::ordered(fm))), true);
    Ok((Created { id: id.to_string(), path, owner_reason: reason, slug_note }, plan))
}

/// `spec new --translation <lang> --from <source>`: a draft with its own key where `[translations]`
/// pairs it (`<slug>-<lang>` beside the source, or under the `<lang>` owner for `layout = "dir"`),
/// with the source's body and chapters copied and lang, rel.translation_of, source_rev recorded.
pub fn translation(c: &Corpus, g: &Graph, from: &str, lang: &str, title: Option<&str>) -> Result<(Created, Plan)> {
    use crate::config::types::TranslationLayout as L;
    if let Some((name, rest)) = crate::remotes::split(&c.cfg, from) {
        bail!(
            "--from {from} is in the remote {name}, which specfold reads but never copies from; copy {rest} into this repository first (or translate it in that repository), then: spec new --translation {lang} --from <the local id>. A local translation may still name a remote source in rel.translation_of; R1 tracks it"
        );
    }
    let Resolution::Doc { idx, .. } = g.resolve(from) else { bail!("--from {from} names no single document") };
    if let Some(orig) = crate::translation::source_ref(c, idx) {
        bail!(
            "--from {} is itself a translation of {orig}; translate the original instead: spec new --translation {lang} --from {orig}",
            c.docs[idx].id
        );
    }
    if crate::translation::lang_problem(lang).is_some() {
        bail!("--translation '{lang}' is not a language tag like ko or pt-br");
    }
    let src = &c.docs[idx];
    let ty = c.cfg.registry.get(&src.id.ty).ok_or_else(|| anyhow!("unknown type '{}'", src.id.ty))?;
    let t = &c.cfg.raw.translations;
    if !t.languages.is_empty() && !t.languages.iter().any(|l| l == lang) {
        bail!("'{lang}' is not in [translations] languages ({}); add it there first", t.languages.join(", "));
    }
    let src_owner = c.owners.by_id(&src.id.owner).ok_or_else(|| anyhow!("no owner '{}'", src.id.owner))?;
    let (owner, slug) = match t.layout {
        L::Dir => {
            let want = if src_owner.path.is_empty() { lang.to_string() } else { format!("{}/{lang}", src_owner.path) };
            let o = c.owners.by_path(&want).ok_or_else(|| {
                anyhow!(
                    "layout = \"dir\" puts the '{lang}' translation in {want}/, which is not an owner: create the directory {want}/ and add [[owners.kinds]] pattern = \"{want}\" to spec.toml"
                )
            })?;
            (o, src.slug.clone())
        }
        // The tag keeps its BCP 47 case in `lang`; in a slug it is lower case (`setup-pt-br`).
        _ => (src_owner, format!("{}-{}", src.slug, lang.to_ascii_lowercase())),
    };
    let mut key = match ty.key {
        KeyKind::Slug => slug.clone(),
        KeyKind::Stamp => stamp::now_stamp(),
    };
    while g.by_id.contains_key(&DocId::new(&ty.name, &owner.id, &key).to_string()) {
        match ty.key {
            KeyKind::Slug => bail!("{}:{}/{key} already exists: the '{lang}' translation is there", ty.name, owner.id),
            KeyKind::Stamp => key = stamp::next_stamp(&key).ok_or_else(|| anyhow!("bad stamp {key}"))?,
        }
    }
    let id = DocId::new(&ty.name, &owner.id, &key);
    let bundle = src.dir.is_some();
    let dir = paths::type_dir(&c.cfg, &c.owners, owner, src.tier, ty);
    let stem = paths::stem(ty, &key, &slug);
    let path = paths::main_file(&dir, ty, &stem, bundle);
    super::forbid::refuse(c, &path, "change [forbid] paths or translate in another place")?;
    let read = |p: &str| std::fs::read_to_string(c.cfg.root.join(p)).map_err(|e| anyhow!("cannot read {p}: {e}"));
    let rev = crate::translation::rev_of(c, idx).ok_or_else(|| anyhow!("cannot read {}", src.path))?;
    let today = stamp::now_value();
    let mut fm = Map::new();
    fm.insert("id".into(), Value::String(id.to_string()));
    let title = title.map(str::to_string).or_else(|| src.title.clone()).unwrap_or_else(|| src.slug.replace('-', " "));
    fm.insert("title".into(), Value::String(title.trim().to_string()));
    // A translation starts as a draft where the type has one, whatever the source's status.
    let status = if ty.statuses.iter().any(|s| s == "draft") { "draft".to_string() } else { ty.initial.clone() };
    fm.insert("status".into(), Value::String(status));
    fm.insert("created".into(), Value::String(today.clone()));
    fm.insert("updated".into(), Value::String(today));
    fm.insert("lang".into(), Value::String(lang.to_string()));
    fm.insert(crate::translation::KEY.into(), Value::String(rev));
    fm.insert("rel".into(), json!({ crate::translation::RELATION: [src.id.to_string()] }));
    let mut plan = Plan::default();
    let body = crate::markdown::frontmatter::split(&read(&src.path)?).body.to_string();
    plan.write(&path, format!("{}{body}", render(&edit::ordered(fm))), true);
    if let Some(src_dir) = &src.dir {
        let new_dir = path.rsplit_once('/').map(|x| x.0).unwrap_or("").to_string();
        for ch in &src.chapters {
            let rest = ch.strip_prefix(&format!("{src_dir}/")).unwrap_or(ch);
            plan.write(format!("{new_dir}/{rest}"), read(ch)?, true);
        }
        for f in &src.bundle_files {
            let rest = f.strip_prefix(&format!("{src_dir}/")).unwrap_or(f);
            plan.copy(f.clone(), format!("{new_dir}/{rest}"));
        }
    }
    let reason = format!("the '{lang}' translation of {}", src.id);
    Ok((Created { id: id.to_string(), path, owner_reason: reason, slug_note: None }, plan))
}

/// The type's template plus a heading for each required section it lacks, so a new document
/// passes S7 from the start.
fn with_required_sections(template: &str, required: &[String]) -> String {
    let have: Vec<String> = template
        .lines()
        .filter(|l| l.starts_with('#'))
        .map(|l| l.trim_start_matches('#').trim().to_lowercase())
        .collect();
    let mut out = template.to_string();
    for s in required {
        let s = s.trim();
        if s.is_empty() || have.contains(&s.to_lowercase()) {
            continue;
        }
        if !out.is_empty() && !out.ends_with("\n\n") {
            out.push_str(if out.ends_with('\n') { "\n" } else { "\n\n" });
        }
        out.push_str(&format!("## {s}\n"));
    }
    out
}

#[cfg(test)]
mod required_sections_tests {
    #[test]
    fn missing_required_sections_are_appended_once() {
        let req = vec!["Decision".to_string(), "Risks".to_string()];
        assert_eq!(
            super::with_required_sections("## Context\n\n## Decision\n", &req),
            "## Context\n\n## Decision\n\n## Risks\n"
        );
        assert_eq!(super::with_required_sections("", &req), "## Decision\n\n## Risks\n");
    }
}
