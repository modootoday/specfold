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

mod links;
use links::{canonical_links, link_replacements, rewrite_links, without_section};

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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// What a status pattern named as the replacing record, resolved after every row is built.
    #[serde(skip)]
    pub replaced_by: Option<String>,
    /// The old Markdown file, whose relative links are rewritten to the new paths.
    #[serde(skip)]
    pub old_main: String,
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
            ..Row::default()
        });
    }
    canonical_links(&mut out, &by_source);
    link_replacements(&mut out);
    rewrite_links(&mut out, &by_source);
    Ok(out)
}

/// Candidate names for a `by` text: link targets (`[ADR-0002](0002-use-y.md)` → `0002-use-y.md`,
/// `0002-use-y`), the text itself, and its trailing number (`ADR-0002` → `0002`).
fn by_candidates(by: &str) -> Vec<String> {
    static LINK: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"\[([^\]]*)\]\(([^)\s]+)\)").expect("static regex is valid"));
    let mut out = vec![];
    let mut texts = vec![];
    for c in LINK.captures_iter(by) {
        let t = c[2].trim_start_matches("./").to_string();
        out.push(stem_of(&t).to_string());
        out.push(t);
        texts.push(c[1].to_string());
    }
    let plain = LINK.replace_all(by, "").trim().to_string();
    texts.push(plain);
    for t in texts {
        let t = t.trim().trim_end_matches(['.', ',']).to_string();
        if t.is_empty() {
            continue;
        }
        let digits: String =
            t.chars().rev().take_while(char::is_ascii_digit).collect::<Vec<_>>().into_iter().rev().collect();
        out.push(t);
        if !digits.is_empty() {
            out.push(digits);
        }
    }
    out
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
    let mut old = old;
    let body_links = if generated { vec![] } else { merge_body_meta(split.body, map, &mut old, &mut unresolved) };
    if ty.fields.iter().any(|f| f == "decision_makers") {
        let body = if generated { "" } else { split.body };
        super::body_meta::people(&mut old, body);
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
    let created = conv.fm.get("created").and_then(Value::as_str).map(str::to_string);
    let src =
        key::Source { path: &u.main, caps: caps.as_ref(), created: created.as_deref(), title: &title, name_stem: name };
    let (k, slug) = key::choose(&c.cfg.root, rule, ty, &src);
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
    let k = unique_key(ty, &owner.id, k, taken);
    let slug = if ty.key == KeyKind::Slug { k.clone() } else { slug };
    let id = DocId::new(&ty.name, &owner.id, &k);
    taken.insert(id.to_string());
    let bundle = !u.chapters.is_empty() || !u.attachments.is_empty() || ty.form == Form::Dir;
    let dir = paths::type_dir(&c.cfg, &c.owners, owner, Tier::Committed, ty);
    let main = paths::main_file(&dir, ty, &paths::stem(ty, &k, &slug), bundle);
    let bundle_dir = main.rsplit_once('/').map(|x| x.0.to_string()).unwrap_or_default();
    let base = if u.source == u.main { u.main.trim_end_matches(".md").to_string() } else { u.source.clone() };
    let files = move_files(c, u, &base, &bundle_dir, out)?;
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
    let mut notes = conv.notes;
    let mut body = if generated { format!("\n# {title}\n") } else { split.body.to_string() };
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
    notes.extend(body_notes(map, &body, generated, scanned.first_heading.as_deref(), named, &slug, &id));
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
        notes,
        replaced_by: conv.replaced_by,
        old_main: if generated { String::new() } else { u.main.clone() },
    })
}

/// Fills status and date from the body where the frontmatter lacks them; returns the body's typed links.
fn merge_body_meta(
    body: &str,
    map: &ImportMap,
    old: &mut Map<String, Value>,
    unresolved: &mut Vec<String>,
) -> Vec<(String, String)> {
    let found = super::body_meta::scan(body, &map.body);
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
fn unique_key(ty: &crate::model::doctype::TypeDef, owner: &str, mut k: String, taken: &BTreeSet<String>) -> String {
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

/// Plans the move of a unit's chapters and attachments into the bundle; returns the `files` entries.
fn move_files(c: &Corpus, u: &Unit, base: &str, bundle_dir: &str, out: &mut ImportPlan) -> Result<Vec<Value>> {
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
    Ok(files)
}

/// Notes on what the imported body still repeats and on a slug that lost its words.
fn body_notes(
    map: &ImportMap,
    body: &str,
    generated: bool,
    heading: Option<&str>,
    named: &str,
    slug: &str,
    id: &DocId,
) -> Vec<String> {
    let mut notes = vec![];
    // C118: say what the body still repeats, rather than rewrite prose the user wrote.
    if !generated {
        let kept = super::body_meta::copied_bullets(body);
        if !kept.is_empty() {
            notes.push(format!(
                "the body keeps {} next to the same values in frontmatter; delete those lines so they cannot drift",
                kept.iter().map(|k| format!("'{k}:'")).collect::<Vec<_>>().join(", ")
            ));
        }
        if let Some(h) = heading
            && map.strip_title_number
            && strip_number(h) != h
        {
            notes.push(format!("the title drops the number, but the body's heading keeps '# {h}'"));
        }
    }
    // As with `spec new`: a mostly non-ASCII name loses its words from the slug.
    let alnum = named.chars().filter(|c| c.is_alphanumeric() && !c.is_ascii_digit()).count();
    let ascii = named.chars().filter(|c| c.is_ascii_alphabetic()).count();
    if alnum > 0 && ascii * 2 < alnum {
        notes.push(format!("the name is mostly non-ASCII, so the slug is '{slug}' (the title stays); to rename: spec mv {} --slug <slug> --write", crate::shell::word(id)));
    }
    notes
}

/// `2. Use X` → `Use X`; a title that is only a number stays.
fn strip_number(title: &str) -> String {
    let digits = title.chars().take_while(char::is_ascii_digit).count();
    match title[digits..].strip_prefix('.') {
        Some(rest) if digits > 0 && !rest.trim().is_empty() => rest.trim().to_string(),
        _ => title.to_string(),
    }
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

#[cfg(test)]
mod tests {
    #[test]
    fn record_numbers_come_off_titles() {
        assert_eq!(super::strip_number("2. Use X"), "Use X");
        assert_eq!(super::strip_number("12. "), "12. ");
        assert_eq!(super::strip_number("2026 plan"), "2026 plan");
    }

    #[test]
    fn sections_come_out_and_links_are_made_relative() {
        let body = "\n# T\n\n## Status\n\nAccepted\n\n## Context\n\nx\n";
        assert_eq!(super::without_section(body, "status").unwrap(), "\n# T\n\n## Context\n\nx\n");
        assert_eq!(super::links::relative(".spec/adr", ".spec/adr/a.adr.md"), "a.adr.md");
        assert_eq!(super::links::relative("svc/.spec/adr", ".spec/adr/a.md"), "../../../.spec/adr/a.md");
        assert_eq!(super::links::join("doc/adr", "../x/./y.md").as_deref(), Some("doc/x/y.md"));
        assert_eq!(super::links::join("", "../y.md"), None);
        assert_eq!(
            super::by_candidates("[ADR-0002](0002-use-y.md)"),
            ["0002-use-y", "0002-use-y.md", "ADR-0002", "0002"]
        );
    }
}
