//! Links between imported documents: replacements, canonical ids, and rewritten relative paths.

use super::names::stem_of;
use super::row::one_ending;
use super::types::ImportPlan;
use crate::authoring::Plan;
use crate::model::markdown::frontmatter;
use crate::paths::{join, relative_from_dir};
use regex::Regex;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// Candidate names for a `by` text: link targets (`[ADR-0002](0002-use-y.md)` → `0002-use-y.md`,
/// `0002-use-y`), the text itself, and its trailing number (`ADR-0002` → `0002`).
pub(super) fn by_candidates(by: &str) -> Vec<String> {
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
pub(super) fn link_replacements(out: &mut ImportPlan) {
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
pub(super) fn canonical_links(out: &mut ImportPlan, by_source: &BTreeMap<String, String>) {
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
                if !crate::corpus::graph::DOC_RELATIONS.contains(&key.as_str()) {
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
            let text =
                format!("{}{}", frontmatter::render(&crate::model::markdown::frontmatter::edit::ordered(fm)), body);
            *content = one_ending(text);
            return;
        }
    }
}

fn dir_of(p: &str) -> &str {
    p.rsplit_once('/').map(|x| x.0).unwrap_or("")
}

/// Relative links in imported bodies that named old files now point at the new paths; links to
/// files outside the import stay for L1/L2 to report.
pub(super) fn rewrite_links(out: &mut ImportPlan, by_source: &BTreeMap<String, String>) {
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
            // C180: inline, <angle>, reference-definition and percent-encoded links alike;
            // fenced and inline code stay as written.
            let (text, n) = crate::model::markdown::link_dest::rewrite(content, |target| {
                (!target.contains("://") && !target.starts_with('/'))
                    .then(|| join(&old_dir, target))
                    .flatten()
                    .and_then(|old| by_source.get(&old))
                    .and_then(|id| paths.get(id))
                    .map(|p| relative_from_dir(&new_dir, p))
            });
            *content = text;
            count += n;
        }
        if count > 0 {
            row.notes.push(format!("{count} link(s) to old file names now point at the new paths"));
        }
    }
}
