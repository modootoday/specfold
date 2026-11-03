//! One imported document: its frontmatter converted, its key, owner and path chosen, and the
//! file written in the plan.

use super::files::move_files;
use super::names::{name_of, strip_number};
use super::notes::body_notes;
use super::types::{ImportPlan, Row};
use crate::authoring::Plan;
use crate::corpus::Corpus;
use crate::import::convert::Converted;
use crate::import::map::{ImportMap, SourceRule};
use crate::import::sources::Unit;
use crate::import::{body_meta, convert, key, owner};
use crate::markdown::{body, frontmatter};
use crate::model::doctype::{Form, KeyKind, TypeDef};
use crate::model::{DocId, Tier, stamp};
use crate::placement::paths;
use anyhow::{Result, anyhow};
use regex::{Captures, Regex};
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;

/// An old file read and its frontmatter converted, before the document is placed.
struct Source<'a> {
    generated: bool,
    text: String,
    unresolved: Vec<String>,
    conv: Converted,
    title: String,
    first_heading: Option<String>,
    caps: Option<Captures<'a>>,
    /// The file name without `.md`.
    name: &'a str,
    /// What the name rule's `slug` group took, else the name.
    named: &'a str,
}

/// Where the document goes: its key, slug, id and main file.
struct Placed {
    k: String,
    slug: String,
    id: DocId,
    owner_reason: String,
    main: String,
}

pub(super) fn doc_row(
    c: &Corpus,
    map: &ImportMap,
    u: &Unit,
    taken: &mut BTreeSet<String>,
    out: &mut ImportPlan,
) -> Result<Row> {
    let rule = &map.sources[u.rule];
    let ty_name = rule.ty.clone().ok_or_else(|| anyhow!("source {} needs a type", rule.glob))?;
    let ty = c.cfg.registry.get(&ty_name).ok_or_else(|| anyhow!("unknown type {ty_name}"))?;
    let mut src = read_source(c, map, u, ty)?;
    let p = place(c, u, rule, ty, &src, taken)?;
    let bundle_dir = p.main.rsplit_once('/').map(|x| x.0.to_string()).unwrap_or_default();
    let base = if u.source == u.main { u.main.trim_end_matches(".md").to_string() } else { u.source.clone() };
    let files = move_files(c, u, &base, &bundle_dir, out)?;
    let (fm, aliases) = frontmatter_of(&mut src, &p, files);
    let mut notes = std::mem::take(&mut src.conv.notes);
    let body = body_of(map, &src, &p, &mut notes);
    let content = format!("{}{}", frontmatter::render(&crate::markdown::frontmatter::edit::ordered(fm)), body);
    crate::authoring::forbid::refuse(c, &p.main, crate::authoring::forbid::NO_LOCAL_HINT)?;
    out.plan.write(p.main.clone(), one_ending(content), true);
    out.plan.remove(u.source.clone());
    // Old paths of chapters and attachments name the document too, so their citations resolve.
    for old in std::iter::once(u.source.clone())
        .chain((u.main != u.source).then(|| u.main.clone()))
        .chain(u.chapters.iter().cloned())
        .chain(u.attachments.iter().cloned())
        .chain(aliases.iter().cloned())
    {
        out.alias_rows.push((old, p.id.to_string()));
    }
    Ok(Row {
        source: u.source.clone(),
        kind: "doc".into(),
        id: p.id.to_string(),
        path: p.main,
        owner_reason: p.owner_reason,
        status_from: src.conv.status_from,
        status_to: fm_status(&out.plan, &p.id.to_string()),
        aliases,
        unresolved: src.unresolved,
        notes,
        replaced_by: src.conv.replaced_by,
        old_main: if src.generated { String::new() } else { u.main.clone() },
    })
}

/// Reads the old file (none for a wrapped unit) and converts its frontmatter.
fn read_source<'a>(c: &Corpus, map: &ImportMap, u: &'a Unit, ty: &TypeDef) -> Result<Source<'a>> {
    let rule = &map.sources[u.rule];
    // A wrapped unit has no Markdown of its own; its document is generated around the files.
    let generated = u.main.is_empty();
    let text = if generated { String::new() } else { std::fs::read_to_string(c.cfg.spec_home.join(&u.main))? };
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
    let mut old = old;
    let body_links = if generated { vec![] } else { merge_body_meta(split.body, map, &mut old, &mut unresolved) };
    if ty.fields.iter().any(|f| f == "decision_makers") {
        let body = if generated { "" } else { split.body };
        body_meta::people(&mut old, body);
    }
    let scanned = body::scan(split.body, split.body_line, &body::path_cite_re(c.cfg.spec_dir()));
    let name = name_of(&u.source).trim_end_matches(".md");
    let name_re = rule.name.as_deref().map(Regex::new).transpose().map_err(|e| anyhow!("bad name regex: {e}"))?;
    let caps = name_re.as_ref().and_then(|r| r.captures(name_of(&u.source)));
    let named = caps.as_ref().and_then(|c| c.name("slug")).map(|m| m.as_str()).unwrap_or(name);
    let heading = scanned.first_heading.clone().map(|h| if map.strip_title_number { strip_number(&h) } else { h });
    let fallback_title = heading.unwrap_or_else(|| named.replace(['-', '_'], " "));
    let mut conv = convert::frontmatter(map, rule, &old, &ty.statuses, &fallback_title);
    unresolved.append(&mut conv.unresolved);
    add_body_links(&mut conv.fm, body_links);
    let title = conv.fm.get("title").and_then(Value::as_str).unwrap_or(&fallback_title).to_string();
    let first_heading = scanned.first_heading;
    Ok(Source { generated, text, unresolved, conv, title, first_heading, caps, name, named })
}

/// Chooses the key and owner, takes the id, and names the main file.
fn place(
    c: &Corpus,
    u: &Unit,
    rule: &SourceRule,
    ty: &TypeDef,
    src: &Source,
    taken: &mut BTreeSet<String>,
) -> Result<Placed> {
    let created = src.conv.fm.get("created").and_then(Value::as_str).map(str::to_string);
    let key_src = key::Source {
        path: &u.main,
        caps: src.caps.as_ref(),
        created: created.as_deref(),
        title: &src.title,
        name_stem: src.name,
    };
    let (k, slug) = key::choose(&c.cfg.spec_home, rule, ty, &key_src);
    let governs: Vec<String> = src
        .conv
        .fm
        .get("rel")
        .and_then(|r| r.get("governs"))
        .map(|v| crate::model::doc::list_of(Some(v)))
        .unwrap_or_default();
    let (owner, owner_reason) = match rule.owner.as_str() {
        "auto" => {
            let home = c.owners.storage_owner(c.owners.containing(&u.source));
            owner::choose(&c.owners, &governs, &src.text, home)
        }
        fixed => {
            let o = c.owners.by_id(fixed).ok_or_else(|| anyhow!("source {}: no owner '{fixed}'", rule.glob))?;
            (o, format!("fixed to {fixed} by the import map"))
        }
    };
    let k = unique_key(ty, &owner.id, k, taken);
    let slug = if ty.key == KeyKind::Slug { k.clone() } else { slug };
    let id = DocId::new(&ty.name, &owner.id, &k);
    taken.insert(id.to_string());
    let bundle = !u.chapters.is_empty() || !u.attachments.is_empty() || ty.form == Form::Dir;
    let dir = paths::type_dir(&c.cfg, &c.owners, owner, Tier::Committed, ty);
    let main = paths::main_file(&dir, ty, &paths::stem(ty, &k, &slug), bundle);
    Ok(Placed { k, slug, id, owner_reason, main })
}

/// The new frontmatter: the converted one with the id, dates, aliases and files; and the aliases.
fn frontmatter_of(src: &mut Source, p: &Placed, files: Vec<Value>) -> (Map<String, Value>, Vec<String>) {
    let mut fm = std::mem::take(&mut src.conv.fm);
    fm.insert("id".into(), Value::String(p.id.to_string()));
    fm.entry("created").or_insert_with(|| Value::String(stamp::normalize_date(&p.k).unwrap_or_else(stamp::today)));
    let created_v = fm["created"].clone();
    let updated = fm.entry("updated").or_insert(created_v.clone());
    // Old records sometimes date an edit before the file existed; the later date wins.
    if updated.as_str() < created_v.as_str() {
        *updated = created_v;
    }
    let mut aliases = std::mem::take(&mut src.conv.aliases);
    if let Some(a) = src.caps.as_ref().and_then(|c| c.name("alias")) {
        aliases.push(a.as_str().to_string());
    }
    if !aliases.is_empty() {
        fm.insert("aliases".into(), json!(aliases));
    }
    if !files.is_empty() {
        fm.insert("files".into(), Value::Array(files));
    }
    (fm, aliases)
}

/// The new body: the old one, or a title for a wrapped unit, less a status section when asked;
/// with notes on what it still repeats.
fn body_of(map: &ImportMap, src: &Source, p: &Placed, notes: &mut Vec<String>) -> String {
    let title = &src.title;
    let mut body =
        if src.generated { format!("\n# {title}\n") } else { frontmatter::split(&src.text).body.to_string() };
    if let Some(name) = map.body.status.as_deref().and_then(|h| h.strip_prefix("section:"))
        && let Some(rest) = without_section(&body, name)
    {
        if map.body.drop_status_section {
            body = rest;
        } else {
            notes.push(format!(
                "the body keeps '## {name}' next to frontmatter status; [body] drop_status_section = true removes it"
            ));
        }
    }
    notes.extend(body_notes(map, &body, src.generated, src.first_heading.as_deref(), src.named, &p.slug, &p.id));
    body
}

/// Fills status and date from the body where the frontmatter lacks them; returns the body's typed links.
fn merge_body_meta(
    body: &str,
    map: &ImportMap,
    old: &mut Map<String, Value>,
    unresolved: &mut Vec<String>,
) -> Vec<(String, String)> {
    let found = body_meta::scan(body, &map.body);
    // A gap only matters where the frontmatter has no value either.
    let has_status = old.contains_key("status");
    let has_date = old.contains_key("date") || old.contains_key("created");
    if let Some(st) = found.status
        && !old.contains_key("status")
    {
        old.insert("status".into(), Value::String(st));
    }
    if let Some(d) = found.date
        && !old.contains_key("date")
        && !old.contains_key("created")
    {
        old.insert("date".into(), Value::String(d));
    }
    unresolved.extend(found.missed.into_iter().filter(|m| {
        !(has_status && m.starts_with("no status in the body") || has_date && m.starts_with("no date in the body"))
    }));
    found.links
}

/// Adds links found in the body to `rel`, once each.
fn add_body_links(fm: &mut Map<String, Value>, body_links: Vec<(String, String)>) {
    if body_links.is_empty() {
        return;
    }
    let rel = fm.entry("rel").or_insert_with(|| Value::Object(Map::new()));
    if let Value::Object(rel) = rel {
        for (kind, target) in body_links {
            let name = target.rsplit('/').next().unwrap_or(&target).trim_end_matches(".md").to_string();
            if let Value::Array(items) = rel.entry(kind).or_insert_with(|| Value::Array(vec![]))
                && !items.iter().any(|v| v.as_str() == Some(&name))
            {
                items.push(Value::String(name));
            }
        }
    }
}

/// The first key after `k` whose id is not taken yet.
fn unique_key(ty: &TypeDef, owner: &str, mut k: String, taken: &BTreeSet<String>) -> String {
    loop {
        let id = DocId::new(&ty.name, owner, &k).to_string();
        if !taken.contains(&id) {
            break;
        }
        k = match ty.key {
            KeyKind::Stamp => stamp::next_stamp(&k).unwrap_or(k),
            KeyKind::Slug => format!("{k}-2"),
        };
    }
    k
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

/// The body without the `## <name>` section (its heading up to the next heading).
pub(super) fn without_section(body: &str, name: &str) -> Option<String> {
    let is_it = |l: &str| l.starts_with('#') && l.trim_start_matches('#').trim().eq_ignore_ascii_case(name);
    let lines: Vec<&str> = body.split_inclusive('\n').collect();
    let at = lines.iter().position(|l| is_it(l.trim()))?;
    let end = lines[at + 1..].iter().position(|l| l.starts_with('#')).map_or(lines.len(), |e| at + 1 + e);
    Some([&lines[..at], &lines[end..]].concat().concat())
}

/// C243: as C154 keeps an edited file, a source with CRLF anywhere in its body gives a document
/// that is CRLF throughout (the generated frontmatter included); otherwise LF.
pub(super) fn one_ending(content: String) -> String {
    if content.contains("\r\n") { content.replace("\r\n", "\n").replace('\n', "\r\n") } else { content }
}
