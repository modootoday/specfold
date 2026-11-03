//! Moving or renaming a document, and rewriting every reference to it.

use super::plan::Plan;
use super::refs;
use crate::corpus::Corpus;
use crate::model::doctype::KeyKind;
use crate::model::markdown::frontmatter::edit;
use crate::model::{DocId, Tier, stamp};
use crate::workspace::placement::{self, paths};
use anyhow::{Result, anyhow, bail};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Default)]
pub struct Move {
    /// Target owner; computed from governs when absent.
    pub owner: Option<String>,
    /// New slug.
    pub slug: Option<String>,
    /// Target tier: promote a local document into the committed tree, or the reverse.
    pub tier: Option<Tier>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Moved {
    pub from: String,
    pub to: String,
    pub path: String,
    pub references: usize,
}

pub fn plan(c: &Corpus, idx: usize, req: &Move) -> Result<(Moved, Plan)> {
    let d = &c.docs[idx];
    let ty = c.cfg.registry.get(&d.id.ty).ok_or_else(|| anyhow!("unknown type {}", d.id.ty))?;
    let current = c.owners.by_id(&d.id.owner).ok_or_else(|| anyhow!("owner {} no longer exists", d.id.owner))?;
    let owner = match &req.owner {
        Some(o) => c.owners.by_id(o).or_else(|| c.owners.by_path(o)).ok_or_else(|| anyhow!("no owner '{o}'"))?,
        None => {
            placement::decide_local(&c.cfg, &c.owners, &d.rel().remove("governs").unwrap_or_default(), current).owner
        }
    };
    let slug = req.slug.clone().unwrap_or_else(|| d.slug.clone());
    if !stamp::is_slug(&slug) {
        bail!("'{slug}' is not a kebab-case slug");
    }
    let key = if ty.key == KeyKind::Slug { slug.clone() } else { d.id.key.clone() };
    let new_id = DocId::new(&ty.name, &owner.id, &key);
    let tier = req.tier.unwrap_or(d.tier);
    let dir = paths::type_dir(&c.cfg, &c.owners, owner, tier, ty);
    let stem = paths::stem(ty, &key, &slug);
    let new_main = paths::main_file(&dir, ty, &stem, d.dir.is_some());
    if new_main == d.path {
        return Ok((
            Moved { from: d.id.to_string(), to: new_id.to_string(), path: new_main, references: 0 },
            Plan::default(),
        ));
    }
    let hint = if tier == Tier::Local {
        "change [forbid] paths or the type's directory"
    } else {
        "choose another --owner, or keep it local: spec new --local --write, then move the body"
    };
    super::forbid::refuse(c, &new_main, hint)?;
    if c.docs.iter().enumerate().any(|(j, x)| j != idx && x.id == new_id) {
        bail!("{new_id} already exists");
    }
    let mut plan = Plan::default();
    match &d.dir {
        Some(bundle) => plan.mv(bundle.clone(), new_main.rsplit_once('/').map(|x| x.0.to_string()).unwrap_or_default()),
        None => plan.mv(d.path.clone(), new_main.clone()),
    }
    let (old_s, new_s) = (d.id.to_string(), new_id.to_string());
    let references = rewrite_references(c, idx, (&old_s, &new_s), &new_main, tier, &mut plan)?;
    follow_tables(c, idx, (&old_s, &new_s), &new_main, &mut plan)?;
    Ok((Moved { from: old_s, to: new_s, path: new_main, references }, plan))
}

/// Every file whose text names the moved document, rewritten: ids, relations, relative links and
/// path citations. Returns how many other files changed.
fn rewrite_references(
    c: &Corpus,
    idx: usize,
    (old_s, new_s): (&str, &str),
    new_main: &str,
    tier: Tier,
    plan: &mut Plan,
) -> Result<usize> {
    let d = &c.docs[idx];
    let mut references = 0;
    for (j, other) in c.docs.iter().enumerate() {
        if other.tier == Tier::Committed && tier == Tier::Local && j != idx {
            continue;
        }
        for file in std::iter::once(&other.path).chain(&other.chapters) {
            let text = std::fs::read_to_string(c.cfg.spec_home.join(file))?;
            let mut body = refs::rewrite_body(&text, old_s, new_s);
            if j != idx {
                // C137: relative Markdown links to the moved file follow it, fragment kept.
                body = refs::rewrite_relative_links(&body, file, &d.path, new_main);
                if let Some(bundle) = &d.dir {
                    let new_dir = new_main.rsplit_once('/').map(|x| x.0).unwrap_or("");
                    body = refs::rewrite_bundle_links(&body, file, bundle, new_dir);
                }
                // Plain paths to the moved file break with the move; cite the new id instead (L2).
                for cite in other.path_cites.iter().filter(|p| &p.path == file) {
                    if refs::same_path(&cite.item, &d.path) {
                        body = refs::cite_path_as_id(&body, &cite.item, new_s);
                    }
                }
            }
            let is_main = file == &other.path;
            if is_main {
                // E3: only the fields that change are written; quoting elsewhere stays.
                body = edit::rewrite_keeping(&body, &[(old_s, new_s)], |m| {
                    refs::rewrite_rel(m, old_s, new_s);
                    if j == idx {
                        m.insert("id".into(), Value::String(new_s.to_string()));
                        m.insert("updated".into(), Value::String(stamp::now_value()));
                    }
                })?;
            }
            // C224: a referring file counts once, whether its links, its rel lines, or both changed.
            if body != text {
                references += usize::from(j != idx);
                let target = if j != idx {
                    file.clone()
                } else if is_main {
                    new_main.to_string()
                } else {
                    let base = d.dir.as_deref().unwrap_or("");
                    let new_base = new_main.rsplit_once('/').map(|x| x.0).unwrap_or("");
                    file.replacen(base, new_base, 1)
                };
                plan.write(target, body, false);
            }
        }
    }
    Ok(references)
}

/// The baseline and the alias table follow the move.
fn follow_tables(c: &Corpus, idx: usize, (old_s, new_s): (&str, &str), new_main: &str, plan: &mut Plan) -> Result<()> {
    let d = &c.docs[idx];
    // D2: baseline entries follow the moved file or bundle, so its known errors stay held.
    let (from_path, to_path) = match &d.dir {
        Some(bundle) => (bundle.clone(), new_main.rsplit_once('/').map(|x| x.0.to_string()).unwrap_or_default()),
        None => (d.path.clone(), new_main.to_string()),
    };
    if let Some((rel, text)) = crate::check::baseline::follow_move(c, &from_path, &to_path)? {
        plan.write(rel, text, false);
    }
    if let Some(file) = &c.aliases.file
        && let Ok(text) = std::fs::read_to_string(c.cfg.spec_home.join(file))
    {
        let new_text: String = text
            .lines()
            .map(|l| match l.split_once('\t') {
                Some((o, n)) if n.trim() == old_s => format!("{o}\t{new_s}\n"),
                _ => format!("{l}\n"),
            })
            .collect();
        if new_text != text {
            plan.write(file.clone(), new_text, false);
        }
    }
    Ok(())
}
