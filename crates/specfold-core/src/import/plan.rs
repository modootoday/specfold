//! Builds the import plan: one row per source for review, and the file changes that apply it.

use super::convert;
use super::key;
use super::map::ImportMap;
use super::owner;
use super::sources::{self, Unit};
use crate::authoring::Plan;
use crate::corpus::Corpus;
use crate::files::hash;
use crate::markdown::{body, frontmatter};
use crate::model::doctype::{Form, KeyKind};
use crate::model::{DocId, Tier, stamp};
use crate::placement::paths;
use anyhow::{Result, anyhow};
use regex::Regex;
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default, Serialize)]
pub struct Row {
    pub source: String,
    /// `doc` or `redirect`.
    pub kind: String,
    pub id: String,
    pub path: String,
    pub owner_reason: String,
    pub status_from: String,
    pub status_to: String,
    pub aliases: Vec<String>,
    pub unresolved: Vec<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct ImportPlan {
    pub rows: Vec<Row>,
    #[serde(skip)]
    pub plan: Plan,
    pub alias_rows: Vec<(String, String)>,
    /// Attachments over the repository limit: (bundle path, sha256) to upload to the store.
    pub uploads: Vec<(String, String)>,
}

fn name_of(p: &str) -> &str {
    p.rsplit('/').next().unwrap_or(p)
}

/// A file's name without `.md`, or a directory's name.
fn stem_of(p: &str) -> &str {
    name_of(p).trim_end_matches(".md")
}

pub fn build(c: &Corpus, map: &ImportMap) -> Result<ImportPlan> {
    let units = sources::expand(&c.cfg.root, map);
    let mut out = ImportPlan::default();
    let mut taken: BTreeSet<String> = c.docs.iter().map(|d| d.id.to_string()).collect();
    let mut by_source: BTreeMap<String, String> = BTreeMap::new();
    let mut redirects = vec![];
    for u in &units {
        let rule = &map.sources[u.rule];
        if let Some(re) = &rule.redirect {
            redirects.push((u.clone(), Regex::new(re).map_err(|e| anyhow!("bad redirect regex: {e}"))?));
            continue;
        }
        if rule.drop {
            out.plan.remove(u.source.clone());
            out.rows.push(Row { source: u.source.clone(), kind: "drop".into(), ..Row::default() });
            continue;
        }
        let row = doc_row(c, map, u, &mut taken, &mut out)?;
        for p in [&u.main, &u.source].into_iter().chain(&u.chapters).chain(&u.attachments) {
            by_source.insert(p.clone(), row.id.clone());
        }
        if map.stem_aliases {
            out.alias_rows.push((stem_of(&u.source).to_string(), row.id.clone()));
        }
        out.rows.push(row);
    }
    for (u, re) in redirects {
        let text = std::fs::read_to_string(c.cfg.root.join(&u.main)).unwrap_or_default();
        let target = re
            .captures(&text)
            .and_then(|cap| cap.name("target"))
            .map(|m| m.as_str().trim().trim_start_matches("./").to_string());
        let resolved = target.as_ref().and_then(|t| {
            by_source.get(t).cloned().or_else(|| c.docs.iter().find(|d| &d.path == t).map(|d| d.id.to_string()))
        });
        let mut unresolved = vec![];
        match &resolved {
            Some(id) => {
                out.alias_rows.push((u.source.clone(), id.clone()));
                if map.stem_aliases {
                    out.alias_rows.push((stem_of(&u.source).to_string(), id.clone()));
                }
            }
            None => unresolved.push(format!("stub target {} is not imported", target.unwrap_or_else(|| "?".into()))),
        }
        out.plan.remove(u.source.clone());
        out.rows.push(Row {
            source: u.source.clone(),
            kind: "redirect".into(),
            id: resolved.unwrap_or_default(),
            path: String::new(),
            owner_reason: String::new(),
            status_from: String::new(),
            status_to: String::new(),
            aliases: vec![],
            unresolved,
        });
    }
    Ok(out)
}

fn doc_row(c: &Corpus, map: &ImportMap, u: &Unit, taken: &mut BTreeSet<String>, out: &mut ImportPlan) -> Result<Row> {
    let rule = &map.sources[u.rule];
    let ty_name = rule.ty.clone().ok_or_else(|| anyhow!("source {} needs a type", rule.glob))?;
    let ty = c.cfg.registry.get(&ty_name).ok_or_else(|| anyhow!("unknown type {ty_name}"))?;
    // A wrapped unit has no Markdown of its own; its document is generated around the files.
    let generated = u.main.is_empty();
    let text = if generated { String::new() } else { std::fs::read_to_string(c.cfg.root.join(&u.main))? };
    let split = frontmatter::split(&text);
    let mut unresolved = vec![];
    let old = match frontmatter::parse(split.raw) {
        frontmatter::Parsed::Map(m) => m,
        frontmatter::Parsed::None => Map::new(),
        frontmatter::Parsed::Invalid(e) => {
            unresolved.push(format!("old frontmatter does not parse: {e}"));
            Map::new()
        }
    };
    let scanned = body::scan(split.body, split.body_line, &body::path_cite_re(c.cfg.spec_dir()));
    let name = name_of(&u.source).trim_end_matches(".md");
    let name_re = rule.name.as_deref().map(Regex::new).transpose().map_err(|e| anyhow!("bad name regex: {e}"))?;
    let caps = name_re.as_ref().and_then(|r| r.captures(name_of(&u.source)));
    let named = caps.as_ref().and_then(|c| c.name("slug")).map(|m| m.as_str()).unwrap_or(name);
    let fallback_title = scanned.first_heading.clone().unwrap_or_else(|| named.replace(['-', '_'], " "));
    let mut conv = convert::frontmatter(map, rule, &old, &ty.statuses, &fallback_title);
    unresolved.append(&mut conv.unresolved);
    let title = conv.fm.get("title").and_then(Value::as_str).unwrap_or(&fallback_title).to_string();
    let created = conv.fm.get("created").and_then(Value::as_str).map(str::to_string);
    let src =
        key::Source { path: &u.main, caps: caps.as_ref(), created: created.as_deref(), title: &title, name_stem: name };
    let (mut k, slug) = key::choose(&c.cfg.root, rule, ty, &src);
    let governs: Vec<String> = conv
        .fm
        .get("rel")
        .and_then(|r| r.get("governs"))
        .map(|v| crate::model::doc::list_of(Some(v)))
        .unwrap_or_default();
    let (owner, owner_reason) = match rule.owner.as_str() {
        "auto" => {
            let home = c.owners.storage_owner(c.owners.containing(&u.source));
            owner::choose(&c.owners, &governs, &text, home)
        }
        fixed => {
            let o = c.owners.by_id(fixed).ok_or_else(|| anyhow!("source {}: no owner '{fixed}'", rule.glob))?;
            (o, format!("fixed to {fixed} by the import map"))
        }
    };
    loop {
        let id = DocId::new(&ty.name, &owner.id, &k).to_string();
        if !taken.contains(&id) {
            break;
        }
        k = match ty.key {
            KeyKind::Stamp => stamp::next_stamp(&k).unwrap_or(k),
            KeyKind::Slug => format!("{k}-2"),
        };
    }
    let slug = if ty.key == KeyKind::Slug { k.clone() } else { slug };
    let id = DocId::new(&ty.name, &owner.id, &k);
    taken.insert(id.to_string());
    let bundle = !u.chapters.is_empty() || !u.attachments.is_empty() || ty.form == Form::Dir;
    let dir = paths::type_dir(&c.cfg, &c.owners, owner, Tier::Committed, ty);
    let main = paths::main_file(&dir, ty, &paths::stem(ty, &k, &slug), bundle);
    let bundle_dir = main.rsplit_once('/').map(|x| x.0.to_string()).unwrap_or_default();
    let base = if u.source == u.main { u.main.trim_end_matches(".md").to_string() } else { u.source.clone() };
    let mut files = vec![];
    // Chapters and attachments move: a sibling directory is not the unit's source, so removing
    // the source alone would leave them behind.
    for ch in &u.chapters {
        let rel = ch.strip_prefix(&format!("{base}/")).unwrap_or(name_of(ch));
        out.plan.copy(ch.clone(), format!("{bundle_dir}/{}", rel.replace('/', "-")));
        out.plan.remove(ch.clone());
    }
    for a in &u.attachments {
        let rel = a.strip_prefix(&format!("{base}/")).unwrap_or(name_of(a)).to_string();
        let (sha, bytes) = hash::file(&c.cfg.root.join(a))?;
        let store = if bytes > c.cfg.raw.files.inline_max_bytes { "store" } else { "repo" };
        if store == "store" {
            out.uploads.push((format!("{bundle_dir}/{rel}"), sha.clone()));
        }
        out.plan.copy(a.clone(), format!("{bundle_dir}/{rel}"));
        out.plan.remove(a.clone());
        files.push(json!({"path": rel, "sha256": sha, "bytes": bytes, "store": store}));
    }
    let mut fm = conv.fm;
    fm.insert("id".into(), Value::String(id.to_string()));
    fm.entry("created").or_insert_with(|| Value::String(stamp::normalize_date(&k).unwrap_or_else(stamp::today)));
    let created_v = fm["created"].clone();
    let updated = fm.entry("updated").or_insert(created_v.clone());
    // Old records sometimes date an edit before the file existed; the later date wins.
    if updated.as_str() < created_v.as_str() {
        *updated = created_v;
    }
    let mut aliases = conv.aliases;
    if let Some(a) = caps.as_ref().and_then(|c| c.name("alias")) {
        aliases.push(a.as_str().to_string());
    }
    if !aliases.is_empty() {
        fm.insert("aliases".into(), json!(aliases));
    }
    if !files.is_empty() {
        fm.insert("files".into(), Value::Array(files));
    }
    let body = if generated { format!("\n# {title}\n") } else { split.body.to_string() };
    let content = format!("{}{}", frontmatter::render(&crate::markdown::frontmatter::edit::ordered(fm)), body);
    crate::authoring::forbid::refuse(c, &main, crate::authoring::forbid::NO_LOCAL_HINT)?;
    out.plan.write(main.clone(), content, true);
    out.plan.remove(u.source.clone());
    // Old paths of chapters and attachments name the document too, so their citations resolve.
    for old in std::iter::once(u.source.clone())
        .chain((u.main != u.source).then(|| u.main.clone()))
        .chain(u.chapters.iter().cloned())
        .chain(u.attachments.iter().cloned())
        .chain(aliases.iter().cloned())
    {
        out.alias_rows.push((old, id.to_string()));
    }
    Ok(Row {
        source: u.source.clone(),
        kind: "doc".into(),
        id: id.to_string(),
        path: main,
        owner_reason,
        status_from: conv.status_from,
        status_to: fm_status(&out.plan, &id.to_string()),
        aliases,
        unresolved,
    })
}

fn fm_status(plan: &Plan, _id: &str) -> String {
    plan.changes
        .iter()
        .rev()
        .find_map(|c| match c {
            crate::authoring::Change::Write { content, .. } => {
                content.lines().find_map(|l| l.strip_prefix("status: ").map(str::to_string))
            }
            _ => None,
        })
        .unwrap_or_default()
}

/// Appends the alias rows to the table file (creating it), keeping one row per old name.
pub fn alias_table(existing: &str, rows: &[(String, String)]) -> String {
    let mut seen: BTreeSet<String> = existing.lines().map(str::to_string).collect();
    let mut out = if existing.is_empty() {
        "# old\tnew-id  (written by spec import; frozen afterwards)\n".to_string()
    } else {
        existing.to_string()
    };
    for (old, new) in rows {
        let line = format!("{old}\t{new}");
        if seen.insert(line.clone()) {
            out.push_str(&line);
            out.push('\n');
        }
    }
    out
}
