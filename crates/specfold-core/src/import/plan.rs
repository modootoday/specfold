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

/// "superseded by X": the newer record, when this import names it, gets rel.supersedes to the
/// older one; otherwise the row needs a decision, since check would report T1 after the import.
fn link_replacements(out: &mut ImportPlan) {
    let names: BTreeMap<String, String> = out.alias_rows.iter().map(|(o, n)| (o.clone(), n.clone())).collect();
    // A bare record number (`ADR-0002` → `0002`) also names the file that starts with it.
    let by_name = |c: &str| {
        let numbered = format!("{c}-");
        names.get(c).cloned().or_else(|| {
            names
                .iter()
                .find(|(o, _)| {
                    stem_of(o) == c || (c.chars().all(|ch| ch.is_ascii_digit()) && stem_of(o).starts_with(&numbered))
                })
                .map(|x| x.1.clone())
        })
    };
    let paths: BTreeMap<String, String> =
        out.rows.iter().filter(|r| r.kind == "doc").map(|r| (r.id.clone(), r.path.clone())).collect();
    for i in 0..out.rows.len() {
        let Some(by) = out.rows[i].replaced_by.clone() else { continue };
        let old = out.rows[i].id.clone();
        let newer = by_candidates(&by).iter().find_map(|c| by_name(c)).filter(|n| *n != old && paths.contains_key(n));
        match newer {
            Some(n) => {
                edit_frontmatter(&mut out.plan, &paths[&n], |fm| {
                    let rel = fm.entry("rel").or_insert_with(|| Value::Object(Map::new()));
                    if let Value::Object(rel) = rel
                        && let Value::Array(items) = rel.entry("supersedes").or_insert_with(|| Value::Array(vec![]))
                        && !items.iter().any(|v| v.as_str() == Some(&old))
                    {
                        items.push(Value::String(old.clone()));
                    }
                });
                out.rows[i].notes.push(format!("replaced by {n}: rel.supersedes added there"));
            }
            None => out.rows[i].unresolved.push(format!(
                "replaced by {by}, which this import does not include: add rel.supersedes: [{old}] to the newer record"
            )),
        }
    }
}

/// `rel.*` values that name an imported file by its old name (`0002-use-postgres`,
/// `0002-use-postgres.md`; document relations only, never paths) are written as the id it gets, so the
/// link does not depend on the alias; the alias still resolves the old name elsewhere.
fn canonical_links(out: &mut ImportPlan, by_source: &BTreeMap<String, String>) {
    let mut names: BTreeMap<String, String> = BTreeMap::new();
    for (src, id) in
        by_source.iter().map(|(a, b)| (a.as_str(), b)).chain(out.alias_rows.iter().map(|(a, b)| (a.as_str(), b)))
    {
        names.entry(stem_of(src).to_string()).or_insert_with(|| id.clone());
    }
    let lookup = |v: &str| -> Option<String> {
        let v = v.trim();
        if v.contains(':') || v.contains('/') {
            return None;
        }
        names.get(stem_of(v)).cloned()
    };
    let paths: Vec<String> = out.rows.iter().filter(|r| r.kind == "doc").map(|r| r.path.clone()).collect();
    for path in paths {
        edit_frontmatter(&mut out.plan, &path, |fm| {
            let Some(Value::Object(rel)) = fm.get_mut("rel") else { return };
            for (key, v) in rel.iter_mut() {
                if !crate::graph::DOC_RELATIONS.contains(&key.as_str()) {
                    continue;
                }
                match v {
                    Value::String(t) => {
                        if let Some(id) = lookup(t) {
                            *t = id;
                        }
                    }
                    Value::Array(items) => {
                        for item in items.iter_mut() {
                            if let Value::String(t) = item
                                && let Some(id) = lookup(t)
                            {
                                *t = id;
                            }
                        }
                    }
                    _ => {}
                }
            }
        });
    }
}

/// Changes the frontmatter of a document this plan writes.
fn edit_frontmatter(plan: &mut Plan, path: &str, f: impl FnOnce(&mut Map<String, Value>)) {
    for c in plan.changes.iter_mut() {
        if let crate::authoring::Change::Write { path: p, content, .. } = c
            && p == path
        {
            let split = frontmatter::split(content);
            let frontmatter::Parsed::Map(mut fm) = frontmatter::parse(split.raw) else { return };
            f(&mut fm);
            let body = split.body.to_string();
            *content = format!("{}{}", frontmatter::render(&crate::markdown::frontmatter::edit::ordered(fm)), body);
            return;
        }
    }
}

/// `a/b/c.md` relative to directory `a/x` → `../b/c.md`.
fn relative(from_dir: &str, to: &str) -> String {
    let f: Vec<&str> = from_dir.split('/').filter(|s| !s.is_empty()).collect();
    let t: Vec<&str> = to.split('/').filter(|s| !s.is_empty()).collect();
    let common = f.iter().zip(&t).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<&str> = std::iter::repeat_n("..", f.len() - common).collect();
    parts.extend(&t[common..]);
    parts.join("/")
}

/// `docs/adr` + `../x/./y.md` → `docs/x/y.md`; None when it climbs above the root.
fn join(dir: &str, link: &str) -> Option<String> {
    let mut parts: Vec<&str> = dir.split('/').filter(|s| !s.is_empty()).collect();
    for seg in link.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    Some(parts.join("/"))
}

fn dir_of(p: &str) -> &str {
    p.rsplit_once('/').map(|x| x.0).unwrap_or("")
}

/// Relative links in imported bodies that named old files now point at the new paths; links to
/// files outside the import stay for L1/L2 to report.
fn rewrite_links(out: &mut ImportPlan, by_source: &BTreeMap<String, String>) {
    static LINK: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"\]\(([^)\s#]+)(#[^)\s]*)?\)").expect("static regex is valid"));
    let paths: BTreeMap<String, String> =
        out.rows.iter().filter(|r| r.kind == "doc").map(|r| (r.id.clone(), r.path.clone())).collect();
    for row in out.rows.iter_mut().filter(|r| r.kind == "doc" && !r.old_main.is_empty()) {
        let old_dir = dir_of(&row.old_main).to_string();
        let new_dir = dir_of(&row.path).to_string();
        let mut count = 0;
        for c in out.plan.changes.iter_mut() {
            let crate::authoring::Change::Write { path, content, .. } = c else { continue };
            if *path != row.path {
                continue;
            }
            let mut fenced = crate::markdown::Fence::default();
            let mut lines = vec![];
            for l in content.split_inclusive('\n') {
                let t = l.trim_start();
                fenced.step(t);
                if fenced.inside() {
                    lines.push(l.to_string());
                    continue;
                }
                lines.push(
                    LINK.replace_all(l, |c: &regex::Captures| {
                        let target = &c[1];
                        let new = (!target.contains("://") && !target.starts_with('/'))
                            .then(|| join(&old_dir, target))
                            .flatten()
                            .and_then(|old| by_source.get(&old))
                            .and_then(|id| paths.get(id));
                        match new {
                            Some(p) => {
                                count += 1;
                                format!("]({}{})", relative(&new_dir, p), c.get(2).map_or("", |m| m.as_str()))
                            }
                            None => c[0].to_string(),
                        }
                    })
                    .into_owned(),
                );
            }
            *content = lines.concat();
        }
        if count > 0 {
            row.notes.push(format!("{count} link(s) to old file names now point at the new paths"));
        }
    }
}

/// The body without the `## <name>` section (its heading up to the next heading).
fn without_section(body: &str, name: &str) -> Option<String> {
    let is_it = |l: &str| l.starts_with('#') && l.trim_start_matches('#').trim().eq_ignore_ascii_case(name);
    let lines: Vec<&str> = body.split_inclusive('\n').collect();
    let at = lines.iter().position(|l| is_it(l.trim()))?;
    let end = lines[at + 1..].iter().position(|l| l.starts_with('#')).map_or(lines.len(), |e| at + 1 + e);
    Some([&lines[..at], &lines[end..]].concat().concat())
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
    let mut body_links = vec![];
    if !generated {
        let found = super::body_meta::scan(split.body, &map.body);
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
        body_links = found.links;
        unresolved.extend(found.missed.into_iter().filter(|m| {
            !(has_status && m.starts_with("no status in the body") || has_date && m.starts_with("no date in the body"))
        }));
    }
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
    if !body_links.is_empty() {
        let rel = conv.fm.entry("rel").or_insert_with(|| Value::Object(Map::new()));
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
    // C118: say what the body still repeats, rather than rewrite prose the user wrote.
    if !generated {
        let kept = super::body_meta::copied_bullets(&body);
        if !kept.is_empty() {
            notes.push(format!(
                "the body keeps {} next to the same values in frontmatter; delete those lines so they cannot drift",
                kept.iter().map(|k| format!("'{k}:'")).collect::<Vec<_>>().join(", ")
            ));
        }
        if let Some(h) = &scanned.first_heading
            && map.strip_title_number
            && strip_number(h) != *h
        {
            notes.push(format!("the title drops the number, but the body's heading keeps '# {h}'"));
        }
    }
    // As with `spec new`: a mostly non-ASCII name loses its words from the slug.
    let alnum = named.chars().filter(|c| c.is_alphanumeric() && !c.is_ascii_digit()).count();
    let ascii = named.chars().filter(|c| c.is_ascii_alphabetic()).count();
    if alnum > 0 && ascii * 2 < alnum {
        notes.push(format!("the name is mostly non-ASCII, so the slug is '{slug}' (the title stays); to rename: spec mv {} --slug <slug> --write", crate::shell::word(&id)));
    }
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
        assert_eq!(super::relative(".spec/adr", ".spec/adr/a.adr.md"), "a.adr.md");
        assert_eq!(super::relative("svc/.spec/adr", ".spec/adr/a.md"), "../../../.spec/adr/a.md");
        assert_eq!(super::join("doc/adr", "../x/./y.md").as_deref(), Some("doc/x/y.md"));
        assert_eq!(super::join("", "../y.md"), None);
        assert_eq!(
            super::by_candidates("[ADR-0002](0002-use-y.md)"),
            ["0002-use-y", "0002-use-y.md", "ADR-0002", "0002"]
        );
    }
}
